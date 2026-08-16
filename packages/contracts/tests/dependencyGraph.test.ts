import { describe, expect, it } from "vitest";
import { validateProviderGraph } from "../src/dependencyGraph";
import type { ProviderDescriptorV1 } from "../src/index";

const provider = (
  id: string,
  service: `viewit.${string}`,
  requires: ProviderDescriptorV1["requires"] = [],
): ProviderDescriptorV1 => ({
  id,
  packageId: id,
  service,
  contractVersion: 1,
  runtime: "builtin",
  trustClass: "built-in",
  requires,
});

describe("provider dependency graph", () => {
  it("activates dependencies before consumers and deactivates in reverse", () => {
    const graph = validateProviderGraph([
      provider("renderer", "viewit.document.render", [
        { service: "viewit.document.parse", contractVersion: 1 },
      ]),
      provider("parser", "viewit.document.parse"),
    ]);
    expect(graph.issues).toEqual([]);
    expect(graph.activationOrder).toEqual(["parser", "renderer"]);
    expect(graph.deactivationOrder).toEqual(["renderer", "parser"]);
  });

  it("reports missing and incompatible requirements", () => {
    const missing = validateProviderGraph([
      provider("renderer", "viewit.document.render", [
        { service: "viewit.document.parse", contractVersion: 1 },
      ]),
    ]);
    expect(missing.issues).toContainEqual({
      providerId: "renderer",
      kind: "missing",
      service: "viewit.document.parse",
    });

    const parser = { ...provider("parser", "viewit.document.parse"), contractVersion: 2 };
    expect(validateProviderGraph([missingProvider(), parser]).issues[0].kind).toBe(
      "version-mismatch",
    );
  });

  it("detects dependency cycles", () => {
    const graph = validateProviderGraph([
      provider("first", "viewit.test.first", [
        { service: "viewit.test.second", contractVersion: 1 },
      ]),
      provider("second", "viewit.test.second", [
        { service: "viewit.test.first", contractVersion: 1 },
      ]),
    ]);
    expect(graph.issues.filter((issue) => issue.kind === "cycle")).toHaveLength(2);
  });
});

function missingProvider(): ProviderDescriptorV1 {
  return provider("renderer", "viewit.document.render", [
    { service: "viewit.document.parse", contractVersion: 1 },
  ]);
}
