# M0 — Existential spike runbook

**Question M0 answers:** does a Tauri/WebKitGTK webview both **render** and **accept input** inside
gamescope on this NVIDIA box? Everything else in OmniDeck is gated on this (see
`omnideck-build-plan.md` §0/§6). If M0 fails, we pivot the UI to a native engine before building more.

## Build

```bash
cd ~/Projects/omnideck
bun run tauri build --no-bundle      # produces src-tauri/target/release/omnideck
```

## Run — do these in order

```bash
# 1) SANITY (Tier 3): prove the binary renders + takes input on the plain KDE desktop.
./m0-spike.sh desktop

# 2) THE GATE: prove the same inside a nested gamescope (SDL backend) on the RTX 3070.
./m0-spike.sh gamescope
```

## PASS / FAIL checklist (watch the window for each)

| Signal | What proves it | Where |
|--------|----------------|-------|
| **Render** | Bars animate, the cube spins, **fps > 0** climbing | the animated strip up top |
| **Keyboard → webview** | press keys → "Keyboard" panel updates + count climbs | left panel |
| **Gamepad (real path)** | press pad buttons → **gilrs** panel updates + count climbs | center panel (highlighted) |
| **Gamepad (webview path)** | "Web Gamepad API" shows a connected pad + last button | right panel |

- **PASS** = render works in `gamescope` mode **and** at least the **gilrs** panel registers input
  (gilrs reads evdev directly, so it should work even if the webview doesn't get keyboard focus).
- **Most important comparison:** does behavior differ between `desktop` and `gamescope`? If desktop
  works but gamescope is black → it's a gamescope/WebKit-render interaction, not the app.

## If the gamescope window is BLACK

Try, in order (edit `m0-spike.sh` or export before running):
1. `__NV_DISABLE_EXPLICIT_SYNC=1` (uncomment the line in `m0-spike.sh`) — NVIDIA flicker/black.
2. `WEBKIT_DISABLE_DMABUF_RENDERER` / `WEBKIT_DISABLE_COMPOSITING_MODE` are already on; confirm they're exported.
3. Try the wayland backend instead of sdl: `gamescope -W 1280 -H 720 -- <env> omnideck` (known to break
   fullscreen children on NVIDIA #1356, but useful as a comparison data point).
4. Check the terminal for gilrs / WebKit errors (the script does not hide stderr).

## M0b (after M0a passes) — the real session path

M0a above runs *plain* nested gamescope (no `STEAM_GAME` gating). The real Tier-1 session uses
`gamescope-session-plus`, which only shows windows tagged `STEAM_GAME=769`. To test that path:

```bash
paru -S gamescope-session-git          # installs gamescope-fg + the session framework
# then launch the binary via gamescope-fg inside a gamescope -e (steam-integration) instance,
# or set the atom manually:  xprop -id <wid> -f STEAM_GAME 32c -set STEAM_GAME 769
```

Capture the result (which signals passed, any env tweaks needed) — it decides whether we proceed on
Tauri or pivot the UI engine.
