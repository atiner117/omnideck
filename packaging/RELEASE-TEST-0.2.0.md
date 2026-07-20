# 0.2.0 release test — the couch checklist

One real session on the 165 Hz host, DualSense **and** DualShock 4 within reach.
Everything here is the stuff the automated harness *can't* see. Check boxes as you go;
anything that fails or feels wrong, jot one line next to it — that's the 0.2.x list.

Before you start: `sudo ./packaging/install-session.sh` (launcher changed since your
last install), then log out → pick **OmniDeck** at SDDM.

## 1. Boot & display
- [ ] UI appears (no black screen) with the **wave wallpaper** flowing
- [ ] fps meter reads **well above 60** (GPU compositing — expect ~150+ at idle)
- [ ] After the session: `grep "session display mode" ~/.local/state/omnideck/omnideck.$(date +%F).log` says `2560x1440 @ 165 Hz`
- [ ] Navigation feels *snappy* — the original complaint this release set out to kill

## 2. Input — do all of this on BOTH controllers
- [ ] Left stick: up moves **up**, down moves **down** (the DS4 inversion fix)
- [ ] D-pad: all four directions, hold-to-repeat scrolls
- [ ] ✕ launches · ◯ backs out · □ favorites a tile · △ opens Add apps
- [ ] Select opens search · Start goes Home · R1 shows item info
- [ ] Keyboard twins: arrows, Enter, Esc, F, A, /, H, I, P, `?` (help overlay)

## 3. Search
- [ ] `/` or Select opens it; type on the real keyboard — **OSK dims** while typing
- [ ] D-pad drives the OSK (dims back on), stick moves the **result list**
- [ ] Launch a result; nonsense query shows "no library matches" + web-search row

## 4. Games (Steam)
- [ ] Launch a game → fullscreen, **Now Playing card** appears
- [ ] Quit the game → focus returns to OmniDeck on its own, card clears
- [ ] Favorite a game (□) → it shows on Home; recents row populates

## 5. App switcher & chords (launch a PWA like YouTube first)
- [ ] **Guide press** → back to OmniDeck, app *keeps running* (audio continues)
- [ ] **Guide press** again → app returns, exactly where it was
- [ ] **Guide hold (~1 s)** → app closes *while you're still holding*
- [ ] Same three with `Ctrl+Alt+Home` / `Ctrl+Alt+End` on the keyboard
- [ ] Launch TWO apps, then close → **both** die, launcher front (close-all fix)

## 6. Media Library — the headline
- [ ] Media Library tile is first in Movies & TV; opens your real server
- [ ] Continue Watching shows your items **with correct resume %**
- [ ] Drill: TV Shows → a series → season → episode; ◯ walks back up
- [ ] Posters fill in as you scroll
- [ ] **Play a movie** → mpv fullscreen, hardware decode
      (`Shift+I` in mpv → hwdec active, not "no")
- [ ] Guide-hold during playback closes mpv, launcher front, Now Playing clears

### 6b. Auto-generated playback profiles (new — test with `mpv_args` REMOVED from
config.toml **and** `auto_profiles` not set to `false`, so the auto path runs; put your
custom set back afterwards if you prefer it. If `pgrep` shows `--hwdec=auto-safe` and no
`--include=`, the auto path was opted out — not a detection failure)
- [ ] `omnideck mpvprofiles` (terminal) → "vapoursynth mpv: true", renders
      `~/.config/omnideck/mpv-profiles/`, tier line shows the real GPU
- [ ] In-session playback: `pgrep -af mpv` shows `--display-fps-override=165…` and
      `--include=…/omnideck/mpv-profiles/mpv.conf`; the rendered `mpv.conf` header
      says `display 2560x1440 @ 165.0 Hz`
- [ ] **F4 once** (smooth) → mpv stats (`i`, page 1) show the filter output near the
      panel rate (~165 fps, NOT 60 — the display-detection fix)
- [ ] **F4 again** (ultra) → output ~82.5 fps on a 1080p source; watch 15+ min: **audio
      stays in sync** (ultra caps at display/2 above 100 Hz AND a per-CPU pixel-rate
      budget; on a **4K source** it deliberately passes through at native rate — smooth
      still takes those to the full rate). If on LDAC headphones and it drifts:
      pause/unpause resyncs = Bluetooth, not us
- [ ] **Skip around while on ultra** → audio stays in sync (the seek self-heal; used to
      drift silently)
- [ ] **F2** stretch fills the screen and back; **F9** shows the toggle status line
- [ ] **F1** → everything off; compare feel
- [ ] `./packaging/test-profiles.sh` → all `OK` (headless rate check of every filter)

### 6c. Controller in launched apps + hidden-app freeze (new)
- [ ] Launch the Jellyfin PWA (or any web app) → **dpad/left stick moves the focus**,
      A selects, B goes back (log line at boot: "navpad: virtual keyboard/mouse ready")
- [ ] **Right stick moves a mouse pointer**; R2 clicks (hold R2 = press-and-hold);
      L1/R1 scroll a page
- [ ] Guide back to dashboard while a SILENT app is open → power draw drops to idle
      within seconds (log: "switcher: froze silent hidden group"); Guide again →
      app resumes exactly where it was
- [ ] Start music (YT Music / Feishin), Guide to dashboard → **music keeps playing**
      (audible apps are never frozen); Guide-hold still closes a hidden frozen app
- [ ] Dashboard idle power/fps re-check: with no apps launched, is the UI smooth and
      the draw sane? (If not: comment `OMNIDECK_GPU_COMPOSITING=1` out of session.conf
      and compare — the GPU-compositing win was validated on the previous driver)

## 7. Settings — walk every section
- [ ] Section headers (Appearance/Background/…) — navigation skips them cleanly
- [ ] **Live wallpaper: Off** → waves gone instantly; back **On** → return
- [ ] **Ambient music: on** → the pad fades in; volume row appears; adjust it;
      verdict on the *mood* (retunable) — then off → fades out
- [ ] Accent color change → rail/waves recolor live
- [ ] UI scale nudge up/down → layout stays sane

## 8. Power & stability
- [ ] **Suspend** → resume → session alive, controller reconnects, clock correct
- [ ] Restart/Shut down ask for confirm; cancel works
- [ ] Leave it idle 10 min → no crash, waves still moving, fps steady
- [ ] **Exit OmniDeck** → clean return to SDDM (note if gamescope segfault-noise
      appears in the journal — known cosmetic, upstream)

## Verdict
- [ ] Everything above passes → say "tag it" and 0.2.0 ships (RELEASING.md §2–4)
- [ ] Anything failed → bring the line notes; fixes become 0.2.0 blockers or 0.2.1
