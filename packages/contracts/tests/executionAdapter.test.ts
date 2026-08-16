import { describe, expect, it } from "vitest";
import {
  ANDROID_IN_PROCESS_RUNTIME,
  EXTERNAL_WORKER_RUNTIME,
  validateExecutionRequest,
} from "../src/executionAdapter";

describe("execution adapter contracts", () => {
  it("describes current Android plugins honestly as uncontained and restart-bound", () => {
    expect(ANDROID_IN_PROCESS_RUNTIME).toMatchObject({
      trustClass: "trusted-in-process",
      unload: "restart-required",
      crashContainment: "none",
      cancellation: "cooperative",
    });
  });

  it("requires process containment before declaring an isolated worker", () => {
    expect(EXTERNAL_WORKER_RUNTIME).toMatchObject({
      trustClass: "isolated",
      crashContainment: "process",
      cancellation: "terminate",
    });
  });

  it("rejects expired and oversized requests before adapter execution", () => {
    const request = {
      id: "request",
      service: "viewit.document.parse" as const,
      contractVersion: 1,
      payload: {},
      deadlineMs: Date.now() + 1000,
    };
    expect(() =>
      validateExecutionRequest(ANDROID_IN_PROCESS_RUNTIME, request, 64 * 1024 * 1024 + 1),
    ).toThrow("exceeds");
    expect(() =>
      validateExecutionRequest(ANDROID_IN_PROCESS_RUNTIME, { ...request, deadlineMs: 0 }, 1),
    ).toThrow("expired");
  });
});
