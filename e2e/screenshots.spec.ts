// A guided tour of the UI, captured as PNGs in e2e/screenshots/<project>/.
//
// Run it: `bun run screenshots` (chromium) or `bun run screenshots:webkit`.
//
// Every test drives the real components — the only fake is the Tauri IPC layer
// (e2e/mock/tauri.ts), which serves the fixture machine in e2e/mock/fixtures.ts. Each test
// also asserts the app never invoked a command the mock doesn't implement, so a new Rust
// command shows up as a failing test instead of a silently-empty screenshot.
import { expect, test, type Page } from "@playwright/test";
import { emit, installTauriMock, unmockedCommands } from "./mock/tauri";
import { LIVE_APPS, PLAYING, defaultBackend, type Backend } from "./mock/fixtures";

/** `navGate()` in +page.svelte drops arrow input within 100ms of the last move. */
const NAV_GATE_MS = 120;

/** Install the mock, load the app, and wait for the boot commands to land. */
async function boot(page: Page, overrides: Partial<Backend> = {}): Promise<void> {
  await installTauriMock(page, overrides);
  await page.goto("/");
  // The "Loading…" toast clears when get_config resolves; a rail row (or the empty-state
  // line, or the wizard) means the first render past boot is on screen.
  await expect(page.locator(".toast")).toHaveCount(0);
  await expect(page.locator(".xitem, .gtile, .lrow, .xempty, .wizard").first()).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
}

/** Press an arrow key n times, spaced past the nav gate. */
async function nav(page: Page, key: string, times = 1): Promise<void> {
  for (let i = 0; i < times; i++) {
    await page.keyboard.press(key);
    await page.waitForTimeout(NAV_GATE_MS);
  }
}

async function shot(page: Page, name: string): Promise<void> {
  const project = test.info().project.name;
  await page.screenshot({ path: `e2e/screenshots/${project}/${name}.png`, animations: "disabled" });
}

test.afterEach(async ({ page }) => {
  // Empty unless the frontend grew a command the harness hasn't taught the mock about.
  expect(await unmockedCommands(page)).toEqual([]);
});

test.describe("library views", () => {
  test("01 home dashboard", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("h"); // Home: pinned favorites, then recents
    await expect(page.locator(".xcat.sel .xclabel")).toHaveText("Home");
    await shot(page, "01-home-dashboard");
  });

  test("02 games rail", async ({ page }) => {
    await boot(page); // Games is the category the app opens on
    await expect(page.locator(".xcat.sel .xclabel")).toHaveText("Games");
    await shot(page, "02-games-rail");
  });

  test("03 games rail, scrolled", async ({ page }) => {
    await boot(page);
    await nav(page, "ArrowDown", 4); // the rail translates so the focused row sits on top
    await shot(page, "03-games-rail-scrolled");
  });

  test("04 poster grid layout", async ({ page }) => {
    const be = defaultBackend();
    be.config.appearance.layout = "grid";
    await boot(page, { config: be.config });
    await expect(page.locator(".gtile").first()).toBeVisible();
    await shot(page, "04-layout-grid");
  });

  test("05 list layout", async ({ page }) => {
    const be = defaultBackend();
    be.config.appearance.layout = "list";
    await boot(page, { config: be.config });
    await expect(page.locator(".lrow").first()).toBeVisible();
    await shot(page, "05-layout-list");
  });

  test("06 settings", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("p");
    await expect(page.locator(".xcat.sel .xclabel")).toHaveText("Settings");
    await shot(page, "06-settings");
  });
});

test.describe("modals", () => {
  test("07 search", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("/");
    await expect(page.getByRole("dialog")).toBeVisible();
    for (const ch of "hollow") await page.keyboard.press(ch); // typed on the OSK's behalf
    await shot(page, "07-modal-search");
  });

  test("08 add apps (catalog)", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("a");
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "08-modal-catalog");
  });

  test("09 help / controls", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("?");
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "09-modal-help");
  });

  test("10 item info", async ({ page }) => {
    await boot(page);
    await page.keyboard.press("i"); // info sheet for the focused tile
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "10-modal-info");
  });

  test("11 power menu", async ({ page }) => {
    await boot(page);
    await page.getByRole("button", { name: "Power menu" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "11-modal-power");
  });
});

test.describe("media library", () => {
  test("12 jellyfin root", async ({ page }) => {
    await boot(page);
    await nav(page, "ArrowRight"); // Games -> Movies & TV
    await page.locator(".xitem").first().click(); // the synthetic Media Library tile
    await expect(page.getByRole("dialog")).toBeVisible();
    await expect(page.locator(".crow").first()).toBeVisible();
    await shot(page, "12-media-root");
  });

  test("13 drilled into TV Shows", async ({ page }) => {
    await boot(page);
    await nav(page, "ArrowRight"); // Games -> Movies & TV
    await page.locator(".xitem").first().click();
    await page.locator(".crow", { hasText: "TV Shows" }).click();
    await expect(page.locator("#dlg-media")).toHaveText("TV Shows");
    await shot(page, "13-media-tv-shows");
  });
});

test.describe("overlays", () => {
  test("14 now playing transport", async ({ page }) => {
    await boot(page, { nowPlaying: PLAYING });
    await expect(page.locator(".nowplaying").first()).toBeVisible();
    await page.keyboard.press("n"); // L1 on a pad
    await expect(page.locator(".np-transport")).toBeVisible();
    await shot(page, "14-now-playing");
  });

  test("15 deck switcher", async ({ page }) => {
    await boot(page, { liveApps: LIVE_APPS });
    await emit(page, "guide-tap", null); // Guide tap / Ctrl+Alt+Home
    await expect(page.locator("section.deck")).toBeVisible();
    await shot(page, "15-deck-switcher");
  });

  test("16 first-run wizard", async ({ page }) => {
    const be = defaultBackend();
    be.config.settings.onboarded = false;
    await boot(page, { config: be.config });
    await shot(page, "16-wizard-welcome");
  });
});
