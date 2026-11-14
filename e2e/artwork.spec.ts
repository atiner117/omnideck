import { expect, test } from "@playwright/test";
import { defaultBackend } from "./mock/fixtures";
import { installTauriMock, unmockedCommands } from "./mock/tauri";

for (const gameBackgrounds of [true, false]) {
  test(`Lutris wallpaper follows game backgrounds (${gameBackgrounds})`, async ({ page }) => {
    const be = defaultBackend();
    be.library.games = [];
    be.config.apps = [{
      id: "lutris:5", name: "Horizon Forbidden West", icon: "🎮",
      exec: ["lutris", "lutris:rungameid/5"], accent: "#f5a524", category: "games",
      art_hero: "/data/omnideck/art/My Game's (2026)/hero.jpg",
    }];
    be.config.settings.game_backgrounds = gameBackgrounds;
    be.config.settings.app_backgrounds = !gameBackgrounds;
    await installTauriMock(page, be);
    await page.goto("/");
    await expect(page.locator(".xitem.focused .xname")).toContainText("Horizon Forbidden West");
    // Wait beyond the focus debounce: App backgrounds must not override this game's toggle.
    await page.waitForTimeout(250);
    if (gameBackgrounds) {
      await expect(page.locator(".xbg.has")).toHaveCSS("background-image",
        'url("omnideck://localhost/data/omnideck/art/My%20Game%27s%20%282026%29/hero.jpg")');
    } else {
      await expect(page.locator(".xbg.has")).toHaveCount(0);
    }
    await expect(page.locator(".xitem.focused .xemoji")).toHaveText("🎮");
    expect(await unmockedCommands(page)).toEqual([]);
  });
}
