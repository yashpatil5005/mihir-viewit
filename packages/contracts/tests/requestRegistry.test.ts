import { describe, expect, it, vi } from "vitest";
import { BridgeRequestRegistry, RequestRestartRequiredError } from "../src/requestRegistry";

describe("bridge request registry", () => {
  it("routes concurrent progress and terminal responses by request id", async () => {
    const registry = new BridgeRequestRegistry();
    const progress: number[] = [];
    const first = registry.request<string>("first", () => {}, {
      onProgress: (value) => progress.push(value),
    });
    const second = registry.request<string>("second", () => {});

    registry.handle({ id: "first", event: "progress", progress: 0.5 });
    registry.handle({ id: "second", event: "complete", result: "two" });
    registry.handle({ id: "first", event: "complete", result: "one" });

    await expect(first).resolves.toBe("one");
    await expect(second).resolves.toBe("two");
    expect(progress).toEqual([0.5]);
    expect(registry.size).toBe(0);
  });

  it("uses typed restart outcomes and ignores duplicate terminal responses", async () => {
    const registry = new BridgeRequestRegistry();
    const request = registry.request("update", () => {});
    expect(registry.handle({ id: "update", event: "restart-required", error: "restart" })).toBe(
      true,
    );
    expect(registry.handle({ id: "update", event: "complete" })).toBe(false);
    await expect(request).rejects.toBeInstanceOf(RequestRestartRequiredError);
  });

  it("times out and removes abandoned requests", async () => {
    vi.useFakeTimers();
    const registry = new BridgeRequestRegistry();
    const request = registry.request("timeout", () => {}, { timeoutMs: 5 });
    const expectation = expect(request).rejects.toThrow("timed out");
    await vi.advanceTimersByTimeAsync(5);
    await expectation;
    expect(registry.size).toBe(0);
    vi.useRealTimers();
  });
});
