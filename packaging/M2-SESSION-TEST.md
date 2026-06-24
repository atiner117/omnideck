# M2 — gamescope session validation (do at the workstation)

Goal: confirm OmniDeck boots as a real 10‑foot **gamescope session**, shows its UI, launches
a game, and returns cleanly. This is the milestone that proves the whole concept.

## 0. Prereqs (already true on the dev host)
- `gamescope` + `gamescope-session-plus` installed (`paru -S gamescope-session-git`). ✓
- A built binary: `cd ~/Projects/omnideck && bun run tauri build --no-bundle`.
- KMS active (NVIDIA modeset on — confirmed via `/sys/class/drm/card*-*` connectors). ✓

## 1. Install the session
```bash
cd ~/Projects/omnideck
./packaging/install-session.sh        # writes session files to /usr/local/share (sudo)
```

## 2. (Recommended) make game launches fast — Steam in the session
OmniDeck is the session client, so Steam isn't auto‑started; `steam://rungameid` will cold‑start
Steam the first time (slow) and *may* not stamp the window. For a clean test, either:
- launch **Steam Big Picture** tile first (gets Steam running in‑session), then back out and launch a game; **or**
- we add "start Steam silently on session login" to the session file next (TODO if the cold path is flaky).

## 3. Boot it
- Log out → at SDDM, pick the **"OmniDeck"** session → log in.

## 4. What to observe (report back each)
1. **Does the OmniDeck UI appear, or BLACK SCREEN?**
   - Black = the `STEAM_GAME` atom didn't take. The binary tries `xprop -name omnideck -f STEAM_GAME 32c -set STEAM_GAME 769` automatically; if it failed, see §6.
2. **Controller + keyboard** navigate the XMB?
3. **Launch a game** (Games → Enter/✕): does it appear fullscreen?
   - OmniDeck should show a **"Now playing"** card (bottom-right) once Steam reports the
     game running. That card is driven by the new exit watchdog (polls Steam's
     `registry.vdf`), so its appearance confirms the watchdog detected the launch.
4. **Close the game**: does focus **return to OmniDeck**, or does gamescope hang on the last frame?
   - On exit the watchdog emits `app-exited` (the "Now playing" card disappears) and
     **re-stamps `STEAM_GAME=769`** on our window to nudge gamescope to refocus us.
   - Report which happens: (a) clean return to OmniDeck, (b) returns but card lingers,
     (c) gamescope hangs on the last frame. Each is useful data.
5. **Power menu**: top-bar **⏻** (or via overlay) → **Exit OmniDeck** ends the session →
   SDDM. **Suspend** works without confirm; **Restart/Shut down** ask to confirm first.

## 5. Getting out / safety
- **Exit OmniDeck** (Settings) quits → back to SDDM.
- If stuck: `Ctrl+Alt+F3` → log in on the TTY → `loginctl terminate-user $USER` or `sudo systemctl restart sddm`.
- Uninstall the session entirely:
  ```bash
  sudo rm /usr/local/share/gamescope-session-plus/sessions.d/omnideck \
          /usr/local/share/wayland-sessions/gamescope-session-omnideck.desktop
  ```

## 6. If black screen — the STEAM_GAME atom
From a TTY (`Ctrl+Alt+F3`) while the session runs, or note for next iteration:
- The window must carry `STEAM_GAME`. Manual set (find the gamescope X display, often `:1`):
  ```bash
  DISPLAY=:1 xprop -name omnideck -f STEAM_GAME 32c -set STEAM_GAME 769
  ```
- Report whether that makes it appear — that tells us if it's an atom-timing/name issue (then we
  fix the in‑app setter: match window class, retry longer, or set before map).

## 7. Logs
- Session/client stderr → the systemd journal: `journalctl --user -b | grep -iE 'omnideck|gamescope'`
- Look for `[omnideck] gilrs ready`, `STEAM_GAME=769 set`, and any WebKit/gamescope errors.
- Watchdog trail per game launch: `watchdog: '<name>' is running` → `watchdog: '<name>'
  exited — refocusing OmniDeck`. If you see "never reported running", the registry path or
  the running-flag heuristic needs adjusting for this Steam version (note the appid).

## What a PASS looks like
UI shows on boot → controller navigates → game launches fullscreen → returns to OmniDeck on exit →
Exit OmniDeck returns to SDDM. Any step that fails is exactly the useful data to bring back.
