import { describe, expect, it, vi } from "vitest";
import {
  installManifest,
  ensureBridgeDispatch,
  isRestartToApplyError,
  normalizeArchiveBridgeResult,
  normalizeDocumentBridgeResult,
  pluginHealthSummary,
  RestartRequiredError,
  unloadJsPlugin,
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

  it("installs the native dispatcher after an SSR-first module import", () => {
    const queuedWindow: {
      __viewitBridgeQueue?: unknown[];
      __viewitBridgeDispatch?: (payload: unknown) => boolean;
    } = {
      __viewitBridgeQueue: [{ id: "early", event: "error", error: "ignored" }],
    };
    vi.stubGlobal("window", queuedWindow);
    ensureBridgeDispatch();
    expect(queuedWindow.__viewitBridgeQueue).toBeUndefined();
    expect(typeof queuedWindow.__viewitBridgeDispatch).toBe("function");
    vi.unstubAllGlobals();
  });

  it("uses a typed restart-required outcome instead of message matching", () => {
    expect(isRestartToApplyError(new RestartRequiredError("apply after restart"))).toBe(true);
    expect(isRestartToApplyError(new Error("restart the app to apply"))).toBe(false);
  });

  it("unwraps native archive callback payloads at the bridge boundary", () => {
    const manifest = {
      ok: true,
      format: "7z",
      entries: [{ name: "file.txt", size: 4, compressed_size: 4, is_dir: false }],
    };
    expect(normalizeArchiveBridgeResult({ ok: true, manifest })).toBe(manifest);
    expect(normalizeArchiveBridgeResult({ ok: true, result: manifest })).toBe(manifest);
  });

  it("unwraps native document callback payloads at the bridge boundary", () => {
    const document = { kind: "unsupported", format: "docx", reason: "test", suggestion: "none" };
    expect(normalizeDocumentBridgeResult({ id: "request", document })).toBe(document);
    expect(normalizeDocumentBridgeResult({ id: "request", result: document })).toBe(document);
    expect(normalizeDocumentBridgeResult(document)).toBe(document);
  });

  it("removes host-managed JavaScript globals and styles on unload", () => {
    const pluginWindow = { ViewItPlugin__test_plugin: { active: true } };
    const style = { remove: vi.fn() };
    vi.stubGlobal("window", pluginWindow);
    vi.stubGlobal("document", {
      getElementById: vi.fn(() => style),
    });

    unloadJsPlugin("test-plugin");

    expect(pluginWindow).not.toHaveProperty("ViewItPlugin__test_plugin");
    expect(style.remove).toHaveBeenCalledOnce();
    vi.unstubAllGlobals();
  });

  it("summarizes the worst provider health state", () => {
    const plugin = {
      id: "health-plugin",
      name: "Health Plugin",
      version: "1.0.0",
      description: "",
      formats: ["txt"],
      providerHealth: {
        parser: { state: "active", consecutiveFailures: 0, totalFailures: 1, updatedAt: 1 },
        renderer: {
          state: "quarantined",
          consecutiveFailures: 3,
          totalFailures: 3,
          lastFailureKind: "invalid-output",
          updatedAt: 2,
        },
      },
    } satisfies PluginInfo;
    expect(pluginHealthSummary(plugin)).toEqual({
      state: "quarantined",
      failures: 4,
      lastFailureKind: "invalid-output",
    });
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
