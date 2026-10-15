import { describe, expect, it } from "vitest";
import type { Settings } from "./backend";
import { SETTING_DEFS, type CycleDef } from "./settings-defs";

// The Privacy rows are the in-app switch for every optional outbound lookup (README → Privacy).
const row = (key: string) => {
  const d = SETTING_DEFS.find((x) => x.key === key);
  if (!d || d.type !== "cycle") throw new Error(`no cycle row ${key}`);
  return d as CycleDef;
};

describe("Privacy settings rows", () => {
  it("sit under a Privacy header", () => {
    const keys = SETTING_DEFS.map((d) => d.key);
    const hdr = keys.indexOf("hdr-privacy");
    expect(hdr).toBeGreaterThanOrEqual(0);
    expect(keys.slice(hdr + 1, hdr + 3)).toEqual(["siteicons", "updatecheck"]);
  });

  it("toggle fetch_remote_icons", () => {
    const off = { fetch_remote_icons: false } as Settings;
    expect(row("siteicons").value(off)).toBe("off");
    expect(row("siteicons").cycle(off)).toEqual({ fetch_remote_icons: true });
    expect(row("siteicons").value({ fetch_remote_icons: true } as Settings)).toBe("on");
  });

  it("toggle check_updates", () => {
    const on = { check_updates: true } as Settings;
    expect(row("updatecheck").value(on)).toBe("on");
    expect(row("updatecheck").cycle(on)).toEqual({ check_updates: false });
  });
});
