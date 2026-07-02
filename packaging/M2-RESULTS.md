# M2 — gamescope session validation RESULTS

> The procedure is in [`M2-SESSION-TEST.md`](./M2-SESSION-TEST.md). This file is the **record**:
> run the session on real hardware and write down what happened. The session is the existential
> feature — do **not** claim "validated" until at least one row here is a real PASS. Dev fleet is
> all-NVIDIA, so AMD + Intel rows clear the single-vendor blind spot before a 1.0 "validated" claim.

Run for each GPU vendor: log out → SDDM → pick **"OmniDeck"** → log in.

| Check | What a PASS looks like |
|------|------------------------|
| **A. UI renders** | OmniDeck XMB appears (not a black screen) |
| **B. Input** | Controller **and** keyboard navigate categories + items |
| **C. Launch** | A Steam game opens fullscreen; the **"Now playing"** card appears (watchdog saw it) |
| **D. Focus return** | On game exit, focus returns to OmniDeck cleanly (card clears; gamescope doesn't hang on the last frame) |
| **E. Power** | Suspend works; Restart/Shut down confirm then work; a polkit denial now **toasts** (not silent) |

For each run record: result per check (PASS / FAIL / N/A), and for any FAIL the
`journalctl --user -b | grep -iE 'omnideck|gamescope|webkit'` snippet + `XDG_SESSION_TYPE`.

---

## NVIDIA — RTX 3070 · driver 610.43.02 · gamescope session (SDDM)
- Date / tester: 2026-07-02 · atiner
- A. UI renders: **PASS** — session boots from SDDM straight into the OmniDeck XMB.
- B. Input: **PASS (keyboard)** — keyboard navigates categories + items and launches tiles.
  Controller: **PENDING** (gamepad wasn't at hand; re-test B + the Guide button).
- C. Launch + Now Playing: **PARTIAL** — a browser PWA (YouTube Music) launched and opened
  properly fullscreen. Steam-game launch + the Now Playing watchdog card: **PENDING**.
- D. Focus return: **PENDING** — exposed a real gap instead: with the PWA fullscreen and
  focused, there was NO keyboard path back to OmniDeck (Guide button is gamepad-only; the
  Now Playing ↩ button is hidden behind the fullscreen app). Fixed by the global
  Ctrl+Alt+Home grab (src-tauri/src/hotkey.rs) — re-test D with it, plus the gamepad Guide
  button and a Steam game's automatic focus-return on exit.
- E. Power menu: **PENDING**
- Notes / logs: first real session run; A/B prove render + input, which were the two
  historical black-screen risks on NVIDIA.

## AMD — `<gpu model>` · Mesa `<version>` · `<session type>`
- Date / tester:
- A. UI renders: **PENDING**
- B. Input: **PENDING**
- C. Launch + Now Playing: **PENDING**
- D. Focus return: **PENDING**
- E. Power menu: **PENDING**
- Notes / logs:

## Intel — `<gpu model>` · Mesa `<version>` · `<session type>`
- Date / tester:
- A. UI renders: **PENDING**
- B. Input: **PENDING**
- C. Launch + Now Playing: **PENDING**
- D. Focus return: **PENDING**
- E. Power menu: **PENDING**
- Notes / logs:

---

## STEAM_GAME atom — keep or drop?
The atom (`set_steam_game_atom_if_gamescope` / the re-stamp in `watch_steam_game`) is treated as
**load-bearing for focus-return until proven otherwise**. Only after check **D** passes on real
hardware *with the atom logic removed* should it be deleted. Until then: keep it. Record the
finding here if you test it.

## Verdict
- [ ] Session validated on at least NVIDIA (A–E all PASS).
- [ ] AMD covered.
- [ ] Intel covered.

When the NVIDIA row is a real PASS, update the README's status/early-dev wording accordingly.
