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

## NVIDIA — `<gpu model>` · driver `<version>` · `<session type>`
- Date / tester:
- A. UI renders: **PENDING**
- B. Input: **PENDING**
- C. Launch + Now Playing: **PENDING**
- D. Focus return: **PENDING** (clean / card lingers / gamescope hangs)
- E. Power menu: **PENDING**
- Notes / logs:

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
