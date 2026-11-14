// Playwright harness for the frontend-only dev server (`bun run dev` on :1420) — the browser
// half of the "Verify" ladder in CLAUDE.md, sitting between the vitest unit tests and the
// nested-gamescope `packaging/test-session.sh` pre-flight. It exercises the real Svelte code
// against a mocked Tauri IPC surface (e2e/mock/tauri.ts), so layout, navigation, modals and
// settings are verifiable with no window, no controller and no Rust build.
//
// Two engines on purpose: `webkit` is the closest available match to the WebKitGTK 4.1 webview
// Tauri actually uses on Linux (that's what ships), while `chromium` is the fast, stable
// default for iterating. Screenshots land in e2e/screenshots/<project>/.
//
// Playwright's WebKit is built on Ubuntu 24.04 and won't start on a rolling distro until its
// sonames (ICU 74, libxml2.so.2, libflite) are supplied — a one-time
// `./e2e/install-webkit-deps.sh`, which is a no-op once done. `chromium` stays the default
// for speed; reach for webkit when the question is "what does it look like in the shipping
// webview".
import { defineConfig } from "@playwright/test";

// The target panel. The UI scales off viewport units, so the viewport IS the test condition —
// a 1280x720 default would exercise a size no OLED TV ever runs at.
const viewport = { width: 1920, height: 1080 };

export default defineConfig({
  testDir: "e2e",
  outputDir: "e2e/.results",
  // The screenshot run is a guided tour, and a single dev server backs every test — keep it
  // sequential so the shots come out in a readable order.
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    viewport,
    deviceScaleFactor: 1,
    colorScheme: "dark",
    trace: "retain-on-failure",
  },
  // Engine chosen directly rather than via devices["Desktop Safari"]/["Desktop Chrome"], whose
  // descriptors also carry a laptop viewport and — for Safari — a 2x Retina scale factor, which
  // would silently render the webkit shots at 3840x2160 and make the two sets incomparable.
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
  webServer: {
    command: "bun run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
