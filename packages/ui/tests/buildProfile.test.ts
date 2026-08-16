import { afterEach, describe, expect, it, vi } from "vitest";

describe("ViewIt build profile", () => {
  afterEach(() => vi.unstubAllEnvs());

  it("defaults unknown and missing frontend values to development", async () => {
    vi.stubEnv("VITE_VIEWIT_APP_PROFILE", "unknown");
    vi.resetModules();
    const { viewitBuildProfile } = await import("../src/pluginBridge");
    expect(viewitBuildProfile()).toBe("development");
  });

  it.each(["device-test", "production"] as const)("exposes the %s profile", async (profile) => {
    vi.stubEnv("VITE_VIEWIT_APP_PROFILE", profile);
    vi.resetModules();
    const { viewitBuildProfile } = await import("../src/pluginBridge");
    expect(viewitBuildProfile()).toBe(profile);
  });
});
