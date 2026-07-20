import { describe, it, expect } from "vitest";
import { mintLaunchId, baseId } from "./launchId";

describe("launch ids", () => {
  it("mints a unique id per launch and round-trips the tile id", () => {
    const a = mintLaunchId("app-youtube");
    const b = mintLaunchId("app-youtube");
    expect(a).not.toBe(b); // two launches of the same tile must not share an exit key
    expect(baseId(a)).toBe("app-youtube");
    expect(baseId(b)).toBe("app-youtube");
  });

  it("parses the backend's media launch keys too (same `base#seq` shape)", () => {
    expect(baseId("media-abc123#7")).toBe("media-abc123");
  });
});
