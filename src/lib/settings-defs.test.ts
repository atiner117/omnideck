import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  SETTING_DEFS, ACCENTS, SEARCH_MODES, normalizeNum,
  type SettingDef, type CycleDef, type NumDef, type TextDef,
} from "./settings-defs";
import type { Appearance, Settings } from "./backend";

// settings-defs is the ONE table the Settings column walks: the page dispatches on `type`,
// filters on `visible`, formats with `value`, and applies whatever `cycle`/`set` returns.
// Nothing renders it in these tests — the defs are pure over Settings, so the whole table is
// exercisable as data. That includes the structural rules the page *assumes* but never checks
// (unique keys, no orphan section header, an action key the page can dispatch).
//
// blip() is the table's one deliberate side effect (the volume preview). It swallows its own
// errors, so it would pass unmocked in node — mocked here so the preview is assertable.
vi.mock("./sfx", () => ({ blip: vi.fn() }));
import { blip } from "./sfx";

const blipped = vi.mocked(blip);

beforeEach(() => {
  blipped.mockClear();
});

/** A complete, valid config — every field present, mirroring the backend defaults. */
function settings(over: Partial<Settings> = {}): Settings {
  return {
    grid_columns: 6, sort: "recent", show_runtimes: false, accent: "#4cc2ff",
    theme: "omnidark", steamgriddb_key: "", onboarded: true, ui_scale: "medium",
    ui_scale_custom: 1.6, bg_blur: 0, bg_brightness: 0.82,
    search_provider: "https://duckduckgo.com/?q=", search_mode: "duckduckgo",
    sound: true, sound_volume: 0.6, dashboard_recents: 8, recents_show: "both",
    background_default: "color", background_color: "#05070b", background_image: "",
    game_backgrounds: true, app_backgrounds: true, live_wallpaper: "waves",
    ambient: false, ambient_volume: 0.35, pin_hash: "", locked_categories: [],
    check_updates: true, overscan_pct: 0, ...over,
  };
}
const appearance = (layout: string): Appearance => ({ layout });

function def<T extends SettingDef>(key: string, type: SettingDef["type"]): T {
  const d = SETTING_DEFS.find((x) => x.key === key);
  if (!d || d.type !== type) throw new Error(`no ${type} def named "${key}"`);
  return d as T;
}
const cyc = (key: string) => def<CycleDef>(key, "cycle");
const num = (key: string) => def<NumDef>(key, "num");
const txt = (key: string) => def<TextDef>(key, "text");

const NUMS = SETTING_DEFS.filter((d): d is NumDef => d.type === "num");
const CYCLES = SETTING_DEFS.filter((d): d is CycleDef => d.type === "cycle");
const VALUED = SETTING_DEFS.filter(
  (d): d is CycleDef | NumDef | TextDef => d.type === "cycle" || d.type === "num" || d.type === "text",
);

/** What the page's `visibleSettings` does to one row (+page.svelte:288). */
const vis = (d: SettingDef, s: Settings, a?: Appearance) => d.visible?.(s, a) ?? true;
/** Apply a def's patch the way `patchSettings` does — merge into the live config. */
const apply = (s: Settings, patch: Partial<Settings>): Settings => ({ ...s, ...patch });

describe("table integrity", () => {
  it("keys are unique", () => {
    // The page keys rows by `key` and dispatches actions on it; a duplicate would double-render
    // and make `doAction` ambiguous.
    const keys = SETTING_DEFS.map((d) => d.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("every section header is followed by at least one unconditional row", () => {
    // Headers are never filtered out (+page.svelte:289 returns true for them), so a section
    // whose rows are ALL conditional can render as a lone header with empty space under it.
    const sections: { header: string; rows: SettingDef[] }[] = [];
    for (const d of SETTING_DEFS) {
      if (d.type === "header") sections.push({ header: d.key, rows: [] });
      else sections.at(-1)?.rows.push(d);
    }
    expect(SETTING_DEFS[0].type).toBe("header"); // no rows before the first section
    expect(sections.length).toBeGreaterThan(0);
    const orphans = sections.filter((s) => !s.rows.some((r) => !r.visible)).map((s) => s.header);
    expect(orphans).toEqual([]);
  });

  it("action rows are exactly the ones the page can dispatch", () => {
    // `doAction` (+page.svelte:481) is a string switch over these two keys — a new action row
    // added without wiring it there would be a dead button.
    const actions = SETTING_DEFS.filter((d) => d.type === "action").map((d) => d.key);
    expect(actions.sort()).toEqual(["addcustom", "overscan"]);
  });

  it("every valued row renders a string from a plain config", () => {
    const bad = VALUED.filter((d) => typeof d.value(settings()) !== "string").map((d) => d.key);
    expect(bad).toEqual([]);
  });

  it("every editable row yields its default from a missing config", () => {
    // `get` is documented to tolerate `undefined` so the edit <input> can render defaults.
    const bad = NUMS.filter((d) => !Number.isFinite(d.get(undefined))).map((d) => d.key);
    expect(bad).toEqual([]);
    const texts = SETTING_DEFS.filter((d): d is TextDef => d.type === "text");
    expect(texts.filter((d) => typeof d.get(undefined) !== "string").map((d) => d.key)).toEqual([]);
  });
});

describe("normalizeNum", () => {
  it("clamps every numeric row to its own bounds", () => {
    for (const d of NUMS) {
      expect(normalizeNum(d, d.lo - 1000)).toBe(d.lo);
      expect(normalizeNum(d, d.hi + 1000)).toBe(d.hi);
    }
  });

  it("rounds int rows to integers and float rows to 2dp", () => {
    expect(normalizeNum(num("gridcols"), 6.6)).toBe(7);
    expect(normalizeNum(num("blur"), 3.4)).toBe(3);
    expect(normalizeNum(num("recents"), 7.5)).toBe(8);
    expect(normalizeNum(num("bright"), 0.8123)).toBe(0.81);
    expect(normalizeNum(num("custom"), 1.6499999999)).toBe(1.65);
  });

  it("leaves every row's default untouched", () => {
    // A default outside [lo,hi] would make the very first D-pad nudge JUMP instead of step.
    for (const d of NUMS) {
      const v = d.get(undefined);
      expect(normalizeNum(d, v)).toBe(v);
    }
  });

  it("one D-pad step actually moves every numeric row", () => {
    // The knob is dead if the step rounds back to where it started (a 0.01 step on a 2dp
    // rounder, say). Uses the same step the page nudges by: adjustStep ?? step.
    for (const d of NUMS) {
      const start = d.get(undefined);
      const step = d.adjustStep ?? d.step;
      expect(normalizeNum(d, start + step)).not.toBe(start);
      expect(normalizeNum(d, d.lo - step)).toBe(d.lo); // and it can't run below the floor
    }
  });

  it("grid columns floor stays at 1, matching the backend clamp", () => {
    // A floor of 3 here would silently RAISE a hand-edited grid_columns of 1-2 on a decrease.
    const d = num("gridcols");
    expect([d.lo, d.hi]).toEqual([1, 12]);
    expect(normalizeNum(d, 1)).toBe(1);
  });
});

describe("cycle rows", () => {
  it("every cycle row returns to its starting state within one lap", () => {
    // No dead ends and no unreachable values: repeatedly feeding a row's own patch back in
    // must revisit the starting config. `layout` is excluded — it is inert by design.
    for (const d of CYCLES.filter((x) => x.key !== "layout")) {
      const start = settings();
      const startKey = JSON.stringify(start);
      let s = start;
      let laps = 0;
      for (let i = 1; i <= 12; i++) {
        s = apply(s, d.cycle(s));
        if (JSON.stringify(s) === startKey) { laps = i; break; }
      }
      expect(laps, `${d.key} never cycles back`).toBeGreaterThan(0);
    }
  });

  it("the layout row is inert — the page intercepts the key", () => {
    // appearance.layout is not a Settings field: Enter/A is handled in cycleSetting and the
    // value is drawn by the inline LayoutPicker, so the def must not patch Settings.
    expect(cyc("layout").cycle(settings())).toEqual({});
    expect(cyc("layout").value(settings())).toBe("");
  });

  it("theme walks the registry and wraps", () => {
    const d = cyc("theme");
    expect(d.value(settings({ theme: "oled" }))).toBe("OLED Black");
    expect(d.cycle(settings({ theme: "omnidark" }))).toEqual({ theme: "oled" });
    expect(d.cycle(settings({ theme: "deck" }))).toEqual({ theme: "omnidark" }); // wraps
    expect(d.cycle(settings({ theme: "nonsense" }))).toEqual({ theme: "omnidark" }); // restarts
    expect(d.value(settings({ theme: "nonsense" }))).toBe("OmniDark");
  });

  it("size walks the presets; an unknown value continues from medium", () => {
    const d = cyc("size");
    expect(d.value(settings({ ui_scale: "huge" }))).toBe("Huge");
    expect(d.cycle(settings({ ui_scale: "small" }))).toEqual({ ui_scale: "medium" });
    expect(d.cycle(settings({ ui_scale: "custom" }))).toEqual({ ui_scale: "small" }); // wraps
    expect(d.cycle(settings({ ui_scale: "gigantic" }))).toEqual({ ui_scale: "large" });
  });

  it("accent and background colour recover from an unknown value differently", () => {
    // Deliberate asymmetry in the fallback index: an off-list accent (the colour wheel can set
    // any hex) advances to ACCENTS[1], while an off-list background colour restarts at [0].
    expect(cyc("accent").cycle(settings({ accent: "#123456" }))).toEqual({ accent: ACCENTS[1] });
    expect(cyc("accent").cycle(settings({ accent: ACCENTS.at(-1)! }))).toEqual({ accent: ACCENTS[0] });
    expect(cyc("bgcolor").cycle(settings({ background_color: "#123456" })))
      .toEqual({ background_color: "#05070b" });
    expect(cyc("accent").value(settings())).toBe(""); // swatch row, no text
  });

  it("the plain toggles flip and label themselves", () => {
    for (const [key, field] of [
      ["gamebg", "game_backgrounds"], ["appbg", "app_backgrounds"],
      ["runtimes", "show_runtimes"], ["ambient", "ambient"],
    ] as const) {
      const d = cyc(key);
      expect(d.value(settings({ [field]: true }))).toBe("on");
      expect(d.value(settings({ [field]: false }))).toBe("off");
      expect(d.cycle(settings({ [field]: true }))).toEqual({ [field]: false });
      expect(d.cycle(settings({ [field]: false }))).toEqual({ [field]: true });
    }
  });

  it("live wallpaper is a two-state cycle with a non-boolean field", () => {
    const d = cyc("livewp");
    expect(d.value(settings({ live_wallpaper: "waves" }))).toBe("Waves");
    expect(d.value(settings({ live_wallpaper: "off" }))).toBe("Off");
    expect(d.cycle(settings({ live_wallpaper: "waves" }))).toEqual({ live_wallpaper: "off" });
    expect(d.cycle(settings({ live_wallpaper: "off" }))).toEqual({ live_wallpaper: "waves" });
  });

  it("background default, recents and sort cycle in order", () => {
    expect(cyc("bgdefault").value(settings({ background_default: "image" }))).toBe("Custom image");
    expect(cyc("bgdefault").value(settings({ background_default: "?" }))).toBe("Solid color");
    expect(cyc("bgdefault").cycle(settings({ background_default: "image" })))
      .toEqual({ background_default: "color" });
    expect(cyc("recents_show").value(settings({ recents_show: "games" }))).toBe("Games");
    expect(cyc("recents_show").cycle(settings({ recents_show: "apps" })))
      .toEqual({ recents_show: "both" });
    expect(cyc("sort").cycle(settings({ sort: "recent" }))).toEqual({ sort: "alpha" });
    expect(cyc("sort").cycle(settings({ sort: "alpha" }))).toEqual({ sort: "recent" });
  });
});

describe("search provider", () => {
  const d = cyc("search");

  it("walks all six providers and wraps", () => {
    const order = SEARCH_MODES.map((m) => m.mode);
    let s = settings({ search_mode: order[0] });
    const seen = [order[0]];
    for (let i = 0; i < order.length; i++) {
      s = apply(s, d.cycle(s));
      seen.push(s.search_mode);
    }
    expect(seen).toEqual([...order, order[0]]);
  });

  it("a preset always carries its own URL", () => {
    expect(d.cycle(settings({ search_mode: "duckduckgo" })))
      .toEqual({ search_mode: "google", search_provider: "https://www.google.com/search?q=" });
    expect(d.value(settings({ search_mode: "brave" }))).toBe("Brave");
  });

  it("entering searxng clears a preset URL so the URL field starts empty", () => {
    expect(d.cycle(settings({ search_mode: "bing", search_provider: "https://www.bing.com/search?q=" })))
      .toEqual({ search_mode: "searxng", search_provider: "" });
  });

  it("but never clobbers a URL the user typed", () => {
    // searxng -> custom with a self-hosted URL already set: the patch must leave it alone.
    const patch = d.cycle(settings({ search_mode: "searxng", search_provider: "https://searx.lan/?q=" }));
    expect(patch).toEqual({ search_mode: "custom" });
    expect("search_provider" in patch).toBe(false);
  });

  it("an unknown mode restarts the cycle and labels as DuckDuckGo", () => {
    expect(d.value(settings({ search_mode: "yahoo" }))).toBe("DuckDuckGo");
    expect(d.cycle(settings({ search_mode: "yahoo" })).search_mode).toBe("google");
  });

  it("the URL row appears only for the two user-supplied modes", () => {
    const url = txt("searchurl");
    for (const mode of SEARCH_MODES.map((m) => m.mode)) {
      expect(vis(url, settings({ search_mode: mode }))).toBe(mode === "searxng" || mode === "custom");
    }
    expect(url.value(settings({ search_provider: "" }))).toBe("(not set)");
    expect(url.value(settings({ search_provider: "https://searx.lan/?q=" }))).toBe("https://searx.lan/?q=");
  });
});

describe("sound", () => {
  const d = cyc("sound");
  const vol = num("soundvol");

  it("labels the presets, and anything else as Custom", () => {
    expect(d.value(settings({ sound: false, sound_volume: 1.0 }))).toBe("Off"); // off outranks volume
    expect(d.value(settings({ sound: true, sound_volume: 0.3 }))).toBe("Low");
    expect(d.value(settings({ sound: true, sound_volume: 0.6 }))).toBe("Medium");
    expect(d.value(settings({ sound: true, sound_volume: 1.0 }))).toBe("High");
    expect(d.value(settings({ sound: true, sound_volume: 0.5 }))).toBe("Custom");
    expect(d.value(settings({ sound: true, sound_volume: 0.6000005 }))).toBe("Medium"); // fp epsilon
  });

  it("cycles Off -> Low -> Medium -> High -> Off", () => {
    let s = settings({ sound: false, sound_volume: 0 });
    const seen: string[] = [];
    for (let i = 0; i < 4; i++) { s = apply(s, d.cycle(s)); seen.push(d.value(s)); }
    expect(seen).toEqual(["Low", "Medium", "High", "Off"]);
  });

  it("a Custom volume drops back to Off on the next press", () => {
    // Custom isn't in the preset list, so the cycle restarts rather than guessing a neighbour.
    expect(d.cycle(settings({ sound: true, sound_volume: 0.5 }))).toEqual({ sound: false, sound_volume: 0 });
  });

  it("previews the volume when switching ON, and stays silent going Off", () => {
    d.cycle(settings({ sound: false, sound_volume: 0 })); // -> Low
    expect(blipped).toHaveBeenCalledTimes(1);
    blipped.mockClear();
    d.cycle(settings({ sound: true, sound_volume: 1.0 })); // High -> Off
    expect(blipped).not.toHaveBeenCalled();
  });

  it("the volume row appears only on a Custom volume, and previews each nudge", () => {
    expect(vis(vol, settings({ sound: true, sound_volume: 0.6 }))).toBe(false);
    expect(vis(vol, settings({ sound: true, sound_volume: 0.5 }))).toBe(true);
    expect(vis(vol, settings({ sound: false, sound_volume: 0.5 }))).toBe(false); // reads "Off"
    vol.adjusted?.();
    expect(blipped).toHaveBeenCalledTimes(1);
  });

  it("dropping the volume to zero also turns sound off", () => {
    expect(vol.set(0)).toEqual({ sound_volume: 0, sound: false });
    expect(vol.set(0.05)).toEqual({ sound_volume: 0.05, sound: true });
  });

  it("the ambient volume row follows the ambient toggle", () => {
    expect(vis(num("ambientvol"), settings({ ambient: false }))).toBe(false);
    expect(vis(num("ambientvol"), settings({ ambient: true }))).toBe(true);
  });
});

describe("conditional rows", () => {
  it("custom size shows only on the custom preset", () => {
    expect(vis(num("custom"), settings({ ui_scale: "medium" }))).toBe(false);
    expect(vis(num("custom"), settings({ ui_scale: "custom" }))).toBe(true);
  });

  it("grid columns show only in a grid layout", () => {
    const d = num("gridcols");
    for (const [layout, shown] of [["rail", false], ["grid", true], ["grid-compact", true], ["list", false]] as const) {
      expect(vis(d, settings(), appearance(layout))).toBe(shown);
    }
    expect(vis(d, settings(), undefined)).toBe(false); // no appearance section -> rail
  });

  it("the background colour and image rows are mutually exclusive", () => {
    const color = cyc("bgcolor"), image = txt("bgimage");
    expect(vis(color, settings({ background_default: "color" }))).toBe(true);
    expect(vis(image, settings({ background_default: "color" }))).toBe(false);
    expect(vis(color, settings({ background_default: "image" }))).toBe(false);
    expect(vis(image, settings({ background_default: "image" }))).toBe(true);
    // A hand-edited third value hides both; the section keeps its unconditional rows.
    expect(vis(color, settings({ background_default: "sepia" }))).toBe(false);
    expect(vis(image, settings({ background_default: "sepia" }))).toBe(false);
  });

  it("blur and brightness show whenever anything is overlaid", () => {
    const off = { game_backgrounds: false, app_backgrounds: false };
    for (const d of [num("blur"), num("bright")]) {
      expect(vis(d, settings({ ...off, background_default: "color" }))).toBe(false);
      expect(vis(d, settings({ ...off, background_default: "image" }))).toBe(true);
      expect(vis(d, settings({ ...off, game_backgrounds: true }))).toBe(true);
      expect(vis(d, settings({ ...off, app_backgrounds: true }))).toBe(true);
    }
  });
});

describe("value formatting", () => {
  it("carries each row's unit", () => {
    expect(num("custom").value(settings({ ui_scale_custom: 2.25 }))).toBe("2.25×");
    expect(num("blur").value(settings({ bg_blur: 12 }))).toBe("12px");
    expect(num("bright").value(settings({ bg_brightness: 0.825 }))).toBe("83%");
    expect(num("gridcols").value(settings({ grid_columns: 8 }))).toBe("8");
    expect(num("ambientvol").value(settings({ ambient_volume: 0.35 }))).toBe("35%");
  });

  it("recents reads 'off' at zero rather than '0'", () => {
    expect(num("recents").value(settings({ dashboard_recents: 0 }))).toBe("off");
    expect(num("recents").value(settings({ dashboard_recents: 12 }))).toBe("12");
  });

  it("the background image shows its basename", () => {
    const d = txt("bgimage");
    expect(d.value(settings({ background_image: "/media/walls/nebula.png" }))).toBe("nebula.png");
    expect(d.value(settings({ background_image: "" }))).toBe("(none)");
    expect(d.get(settings({ background_image: "/media/walls/nebula.png" }))).toBe("/media/walls/nebula.png");
    // Documented wart: a path with a trailing slash renders blank rather than "(none)" —
    // `split("/").pop()` yields "", which is not nullish so the ?? fallback never fires.
    expect(d.value(settings({ background_image: "/media/walls/" }))).toBe("");
  });
});
