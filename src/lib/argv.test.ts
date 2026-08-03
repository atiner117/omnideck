import { describe, expect, it } from "vitest";
import { splitArgv } from "./argv";

// One test per documented rule in argv.ts — the tokenizer decides word boundaries only,
// so these pin the boundary behavior, not any shell semantics (there are none).
describe("splitArgv", () => {
  it("splits on runs of whitespace", () => {
    expect(splitArgv("mpv --fs video.mkv")).toEqual(["mpv", "--fs", "video.mkv"]);
    expect(splitArgv("  mpv \t --fs  ")).toEqual(["mpv", "--fs"]);
  });

  it("groups quoted spans, double and single", () => {
    expect(splitArgv('"/home/user/My Games/launcher" --big')).toEqual([
      "/home/user/My Games/launcher",
      "--big",
    ]);
    expect(splitArgv("'/path with spaces/bin'")).toEqual(["/path with spaces/bin"]);
  });

  it("allows quotes mid-word (--path=\"/My Games\" is one arg)", () => {
    expect(splitArgv('--path="/My Games"')).toEqual(["--path=/My Games"]);
  });

  it("escapes only \" and \\ inside double quotes", () => {
    expect(splitArgv('"say \\"hi\\""')).toEqual(['say "hi"']);
    expect(splitArgv('"C:\\\\Games"')).toEqual(["C:\\Games"]);
    // Other backslashes inside double quotes are literal (no doubling needed).
    expect(splitArgv('"a\\b"')).toEqual(["a\\b"]);
  });

  it("treats everything inside single quotes as literal", () => {
    expect(splitArgv("'a\\\"b'")).toEqual(['a\\"b']);
  });

  it("escapes the next character outside quotes", () => {
    expect(splitArgv("My\\ Games/run")).toEqual(["My Games/run"]);
    expect(splitArgv("a\\'b")).toEqual(["a'b"]);
  });

  it("keeps an explicit empty arg but drops absent ones", () => {
    expect(splitArgv('cmd ""')).toEqual(["cmd", ""]);
    expect(splitArgv("")).toEqual([]);
    expect(splitArgv("   ")).toEqual([]);
  });

  it("returns null on an unbalanced quote instead of guessing", () => {
    expect(splitArgv('"unterminated')).toBeNull();
    expect(splitArgv("it's broken")).toBeNull();
  });
});
