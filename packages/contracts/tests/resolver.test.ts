import { describe, expect, it } from "vitest";
import { providersForPackages, resolveProvider, type ProviderCandidate } from "../src/resolver";

describe("provider resolver", () => {
  it("selects deterministically regardless of candidate order", () => {
    const candidates = providersForPackages(["office-universal", "iwork-universal"], "installed");
    const request = {
      service: "viewit.document.parse" as const,
      contractVersion: 1,
      format: "docx",
    };

    expect(resolveProvider(request, candidates).selected?.id).toBe("office-universal.parse");
    expect(resolveProvider(request, [...candidates].reverse()).selected?.id).toBe(
      "office-universal.parse",
    );
  });

  it("rejects renderers whose parser dependency is unavailable", () => {
    const [renderer] = providersForPackages(["pptx-vanilla"], "installed");
    const decision = resolveProvider(
      { service: "viewit.document.render", contractVersion: 1, format: "pptx" },
      [renderer],
    );

    expect(decision.selected).toBeNull();
    expect(decision.trace[0].reasons).toContain("missing-dependency");
  });

  it("honors an eligible preference before priority and availability", () => {
    const builtIn: ProviderCandidate = {
      id: "builtin.docx",
      packageId: "viewit",
      service: "viewit.document.parse",
      contractVersion: 1,
      runtime: "builtin",
      trustClass: "built-in",
      formats: ["docx"],
      priority: 10,
      availability: "built-in",
      health: "active",
    };
    const [installed] = providersForPackages(["office-universal"], "installed");

    expect(
      resolveProvider(
        {
          service: "viewit.document.parse",
          contractVersion: 1,
          format: "docx",
          preferredProviderId: builtIn.id,
        },
        [installed, builtIn],
      ).selected?.id,
    ).toBe(builtIn.id);
  });

  it("excludes failed and quarantined providers with trace reasons", () => {
    const [provider] = providersForPackages(["office-universal"], "installed");
    const decision = resolveProvider(
      { service: "viewit.document.parse", contractVersion: 1, format: "docx" },
      [{ ...provider, health: "quarantined" }],
    );

    expect(decision.selected).toBeNull();
    expect(decision.trace[0]).toMatchObject({ eligible: false, reasons: ["unhealthy"] });
  });
});
