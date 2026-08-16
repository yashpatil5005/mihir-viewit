import { describe, expect, it } from "vitest";
import { initialProviderHealth, transitionProviderHealth } from "../src/providerHealth";

describe("provider health state", () => {
  it("tracks activation and clears consecutive failures after success", () => {
    let record = initialProviderHealth("office.parse", 0);
    record = transitionProviderHealth(record, { type: "failure", kind: "activation", at: 1 });
    record = transitionProviderHealth(record, { type: "activate", at: 2 });
    record = transitionProviderHealth(record, { type: "activated", at: 3 });
    expect(record).toMatchObject({ state: "active", consecutiveFailures: 0, totalFailures: 1 });
  });

  it("quarantines after three consecutive failures without deleting history", () => {
    let record = initialProviderHealth("office.parse", 0);
    for (let at = 1; at <= 3; at++) {
      record = transitionProviderHealth(record, { type: "failure", kind: "invalid-output", at });
    }
    expect(record).toMatchObject({
      state: "quarantined",
      consecutiveFailures: 3,
      totalFailures: 3,
      lastFailureKind: "invalid-output",
    });
  });

  it("allows explicit retry to clear quarantine streak", () => {
    let record = {
      ...initialProviderHealth("office.parse", 0),
      state: "quarantined" as const,
      consecutiveFailures: 3,
    };
    record = transitionProviderHealth(record, { type: "retry", at: 4 });
    expect(record).toMatchObject({ state: "inactive", consecutiveFailures: 0 });
  });
});
