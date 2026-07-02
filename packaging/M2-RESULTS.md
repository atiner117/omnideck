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
- B. Input: **PASS** — keyboard navigates categories + items and launches tiles. Controller
  (Bluetooth pad, low battery): D-pad nav + **Guide button returned from the app to
  OmniDeck** before the pad died. Re-test with a charged pad.
- C. Launch + Now Playing: **PARTIAL** — browser PWAs (YouTube Music) launch and open
  properly fullscreen; KDE System Settings launches. Steam-game launch + the Now Playing
  watchdog card: **PENDING**.
- D. Focus return: **PARTIAL** — Guide→home worked on hardware (run 2). Gap found in run 1
  (no keyboard path back from a focused fullscreen app) → fixed: global Ctrl+Alt+Home /
  Ctrl+Alt+End grabs + a real hide/show app switcher (hotkey.rs, switcher.rs). The chord
  handler was verified in the LIVE session via XTEST injection (log receipt); the user's
  physical presses never arrived (suspect flaky keyboard / wrong key — Home is the nav
  cluster, not Super). The unmap→focus-falls-to-OmniDeck→remap→app-refocuses cycle was
  verified live against gamescope. Steam-game automatic focus return: **PENDING**.
- E. Power menu: **PASS (partial)** — Shut down and Log out work from the session. Suspend +
  the polkit-denial toast path: **PENDING**.
- Notes / logs: runs 1–2 (2026-07-02). A/B prove render + input, the two historical
  black-screen risks on NVIDIA. gamescope focus rules (live-verified): ignores
  _NET_ACTIVE_WINDOW and GAMESCOPECTRL_BASELAYER_APPID for plain windows; focus follows
  window mapping — the basis of the switcher.

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
