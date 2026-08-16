import { describe, expect, it } from "vitest";
import { ActivationScope } from "../src/activationScope";

describe("activation scope", () => {
  it("disposes resources in strict reverse order and only once", async () => {
    const scope = new ActivationScope("provider");
    const order: number[] = [];
    scope.register("first", () => order.push(1));
    scope.register("second", async () => order.push(2));

    const [first, second] = await Promise.all([scope.dispose("test"), scope.dispose("again")]);
    expect(order).toEqual([2, 1]);
    expect(first).toBe(second);
    expect(scope.state).toBe("disposed");
  });

  it("cascades to children and reports cleanup failures", async () => {
    const parent = new ActivationScope("app");
    const child = parent.child("provider");
    child.register("broken", () => {
      throw new Error("cleanup failed");
    });

    const report = await parent.dispose("shutdown");
    expect(child.state).toBe("disposed");
    expect(report.failures).toHaveLength(0);
    expect(await child.dispose()).toMatchObject({ failures: [{ label: "broken" }] });
  });
});
