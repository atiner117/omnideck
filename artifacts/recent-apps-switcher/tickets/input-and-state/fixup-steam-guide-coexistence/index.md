---
title: "Steam Guide focus competes with owned-app Home"
kind: ticket
status: draft
created: 2026-09-29
updated: 2026-09-29
thread: t3code/0075e72c
---

# Steam Guide focus competes with owned-app Home

## Live reproduction and impact

On r2d2, the deployed integration branch `deploy/guide-home-20260929` at `17b1857` (installed release SHA256 `98476e9c…`) passed **keyboard** Ctrl+Alt+Home tests with a real Jellyfin PWA. Andrew then tapped the **physical DualSense PS button** while Jellyfin was foreground. The session log recorded `guide: tap — Home / last app` and `switcher: froze …`; gamescope instead presented the Steam Big Picture window (`GAMESCOPE_FOCUSED_WINDOW=50331852`, `_NET_WM_PID=131124`, `STEAM_GAME=769`). The window is not OmniDeck-owned. Repeated PS taps and holds cannot reliably bring OmniDeck over Steam. The live Big Picture Power menu labels the safe non-shutdown action **“Switch to Desktop”**, not “Exit Big Picture Mode”; Sign Out is adjacent and must not be selected. Desktop Steam may still intercept the Guide button, so menu exit alone is an unverified workaround.

Additional read-only observation: the previously launched Jellyfin X window `56623108` still exists, while its old leader PID `194302` is gone and multiple members of that process group were in `T` (SIGSTOP) state. Do not blindly SIGCONT/SIGTERM a guessed PGID or promise that the current MRU registry can restore it; re-probe identities before any recovery action. No Steam game process should be killed or controller-wide device grab installed to solve this.

Steam exposes **Settings → Controller → “Guide Button Focuses Steam”** as a distinct setting from per-game Steam Input (see [Steam discussion](https://steamcommunity.com/discussions/forum/1/4739473745778186997/) and [Valve Linux issue](https://github.com/ValveSoftware/steam-for-linux/issues/4885)). Disabling *only* that focus behavior, then switching Big Picture to desktop, is a reversible user-facing experiment; it has **not** been verified on this r2d2 build and might alter the Steam overlay Guide shortcut. Do not disable Steam Input for PlayStation controllers or individual games; those mappings are needed for gameplay.

## Scope and design boundary

Goal: reliably reach OmniDeck Home/overview from an OmniDeck-owned app even when Steam is running, while Steam Input continues delivering face buttons/sticks to active Steam games. This is a fixup to the first owned-app slice, **not** approval to seize/close Steam games or advertise them as cards. Dependencies: [input and state](../index.md), [spec](../../../index.md) §Product contract/Delivery slices; Steam-game takeover remains [separate](../../steam-game-control/index.md).

- Determine whether Steam's Guide-focus action independently raises Big Picture or whether unmapping Jellyfin merely exposes an already-mapped Steam window (both may occur). Instrument the before/after `GAMESCOPE_FOCUSED_WINDOW` and map states in nested gamescope and on r2d2 only in an idle window.
- Prove a gamescope-supported focus transition to OmniDeck that targets only the launcher/Steam **client** case, not an active game, without mapping/unmapping a Steam game's own window. Treat foreign/Steam focus as a no-op on unsupported paths and report it honestly.
- Preserve deployed r2d2 `STEAM_GAME` exclusion, `SteamWatchGuard`, focused-window/navpad gate, and STOPPED/LEADERLESS bookkeeping. PR #101 source is older than deployed branch; do not install its binary over r2d2 or drop later safeguards while porting.
- If the leader has exited while group members are frozen, offer verified member-identity recovery/close without resurrecting a dead card; no unverified group signal, broad CONT, or process-group kill.

## Acceptance checks

1. With Steam running in desktop mode and Jellyfin focused, physical PS tap selects OmniDeck Home; second tap restores the **same** running Jellyfin window; hold opens only Home + genuinely live owned-app cards. With Big Picture already mapped behind Jellyfin, result is explicit and recoverable; no silent fallback to Steam.
2. During a Steam game, face buttons/sticks continue through Steam Input. Test an explicit safe way to return Home and back; if unsupported, show a clear limitation instead of stealing focus or input. Steam-game close is out of scope.
3. B/Guide cancel, Home selection, leaderless/frozen cleanup, and foreign-window safety pass nested and r2d2 tests. Separate true physical-pad results from keyboard shortcuts; do not claim parity from synthetic chords alone.

Likely files: `src-tauri/src/{gamepad,switcher,watchdog}.rs`, `src/routes/+page.svelte`, `packaging/test-session.sh`, and a Steam coexistence help note. No host setting should be silently changed by OmniDeck.

## Root cause confirmed (2026-09-29, live r2d2 probe)

- Steam's own CEF windows (`steamwebhelper`, PID 131124) are stamped **`STEAM_GAME=769`**, the same "main application" appid OmniDeck stamps on `0x400001`. Steam also writes `GAMESCOPECTRL_BASELAYER_APPID = 413091, 769`. When the switcher unmaps an owned app, gamescope falls back to a 769 window and Steam's (newer) one wins.
- The "Guide Button Focuses Steam" setting is **not present** in this Steam build's UI (Andrew looked); drop it as the workaround.
- Big Picture → **Switch to Desktop** did nothing visible. `steam steam://close/bigpicture` closed BPM, but Steam's desktop "Steam" window (also 769) mapped and took focus; politely closing that with WM_DELETE_WINDOW made Steam **re-open Big Picture** within 3 s (Steam re-asserts BPM inside gamescope).
- **What worked:** `xdotool windowunmap <steam BPM window>` — focus fell back to OmniDeck (`0x400003`) and stayed there (checked after 3 s and 8 s). Steam PID 129798 still running; no game was running.

### Proposed fix
On Guide tap/hold, after hiding owned windows, if the focused window's PID is a Steam **client** process (`steamwebhelper` / Steam main PID, window name "Steam" / "Steam Big Picture Mode") — never a game (reaper/SteamLaunch descendant) — unmap it and track it in `HIDDEN`-like state so it can be re-mapped on demand (e.g. a "Steam" card). Game windows keep gamescope's native focus path; Steam-game Home is still separate (steam-game-control ticket).
