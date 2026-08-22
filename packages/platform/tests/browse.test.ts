import { describe, expect, it } from "vitest";
import { friendlyCrumbs } from "../src/browse";

describe("friendlyCrumbs", () => {
  it("labels emulated internal storage instead of 'sdcard'", () => {
    const crumbs = friendlyCrumbs("/sdcard/Download/testing_viewit");
    expect(crumbs[0]).toEqual({ label: "Internal storage", path: "/sdcard" });
    expect(crumbs.map((c) => c.label)).toEqual(["Internal storage", "Download", "testing_viewit"]);
    expect(crumbs[2].path).toBe("/sdcard/Download/testing_viewit");
  });

  it("labels /storage/emulated/0 as internal storage", () => {
    const crumbs = friendlyCrumbs("/storage/emulated/0/DCIM");
    expect(crumbs.map((c) => c.label)).toEqual(["Internal storage", "DCIM"]);
    expect(crumbs[1].path).toBe("/storage/emulated/0/DCIM");
  });

  it("labels removable cards as SD card", () => {
    const crumbs = friendlyCrumbs("/storage/0000-FFFF/movies");
    expect(crumbs.map((c) => c.label)).toEqual(["SD card", "movies"]);
  });

  it("passes non-storage paths through untouched", () => {
    const crumbs = friendlyCrumbs("/home/user/docs");
    expect(crumbs.map((c) => c.label)).toEqual(["home", "user", "docs"]);
    expect(crumbs[2].path).toBe("/home/user/docs");
  });

  it("handles a bare storage root", () => {
    expect(friendlyCrumbs("/sdcard")).toEqual([{ label: "Internal storage", path: "/sdcard" }]);
  });

  it("ignores trailing slashes", () => {
    const crumbs = friendlyCrumbs("/sdcard/Download/");
    expect(crumbs.map((c) => c.label)).toEqual(["Internal storage", "Download"]);
  });
});
