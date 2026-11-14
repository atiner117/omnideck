import { describe, expect, it } from "vitest";
import { artUrl } from "./art";

describe("local artwork URLs", () => {
  it("encodes filenames without allowing CSS url punctuation to escape", () => {
    expect(artUrl("/home/a/My Game's (2026)/cover #1.jpg"))
      .toBe("omnideck://localhost/home/a/My%20Game%27s%20%282026%29/cover%20%231.jpg");
  });
  it("does not turn remote URLs or relative paths into artwork requests", () => {
    expect(artUrl("https://example.com/cover.jpg")).toBe("");
    expect(artUrl("cover.jpg")).toBe("");
  });
});
