# couchlab — the agent's couch

`packaging/couchlab/couchlab.sh` drives the **real** OmniDeck session on the couch box
(r2d2: NVIDIA + Sony A8F over HDMI, gamescope, SDDM autologin) from any machine with SSH
to it, and brings back evidence an agent can read: gamescope's composited frame, the DRM
plane/CRTC/atom state, every omnideck log line written during the run, and a verdict from
the local vision model. Nothing here touches the app; it is the couch test made scriptable.

```
couchlab.sh state                    # ground truth + risk flags (read-only)
couchlab.sh shot [label]             # composited frame → /tmp/couchlab/<ts>-<label>/
couchlab.sh scenario deck-toggle     # Guide, wait, Guide — with a frame mid-sequence
couchlab.sh seq <label> "<steps>" [shot-offsets]   # any virtual-pad `seq` grammar
couchlab.sh judge <png> [question]   # local VLM (llama-swap on r2d2), never cloud
couchlab.sh watch 120                # poll state; print when a risk flag changes
couchlab.sh report [run-dir]
```

## Risk flags (each is a corruption signature that has been seen on the TV)

| flag | what it means | first seen |
|---|---|---|
| `OVERLAY_PLANE_FB` | a second DRM plane holds a framebuffer → displaced blocks | 2026-09-20 |
| `ATOM_NOT_FORCED` | `GAMESCOPE_COMPOSITE_FORCE` ≠ 1 → direct scanout, wrong tiling | 2026-09-13 |
| `PITCH_MISMATCH` | gamescope root size ≠ CRTC size → 4K buffers on a 1080p CRTC | 2026-09-26 |
| `MODE_MISMATCH` | display-mode file asks for a mode the CRTC is not in | 2026-09-26 |
| `FROZEN_WHILE_APP_IN_FRONT` | a SIGSTOPped group exists while the pad is handed to an app | switcher class |
| `NO_COMPOSITOR` `NO_APP` `NO_KEEPER` `SHIM_NO_DISPLAY` | liveness | — |

`state` clean + `shot` clean + a bad TV = scanout/link/TV, not the app.

## What it cannot see

The TV panel. The composited frame is pre-scanout. The last gap needs a sensor on the
far side of the HDMI link: a camera aimed at the TV, or a USB HDMI capture stick behind a
splitter. None of the household cameras faces the TV wall (checked 2026-09-27).

## Sibling lanes

* `packaging/test-session.sh` — nested gamescope on a desktop (ares): switcher/hotkey/pad
  paths with a deterministic stub app; no real display path.
* r2d2 host tools (HomeLab `hosts/r2d2/`, outside this repo on purpose): `omnideck-debug`
  (snap/record on the box), `omnideck-couchlog` (log shipping), `omnideck-display`
  (mode switch / restart), `omnideck-composite-keeper` (re-asserts the atom).

## First result (2026-09-27)

`scenario deck-toggle` against the live session with the Jellyfin PWA in front: OmniDeck
logged `guide: tap — toggle deck`, froze the hidden Brave group (11 members) and thawed it
on the second tap (no `T` states afterwards) — the switcher did its job. But the frame at
t+1.2 s was **Steam Big Picture's side menu with "Controller Connected — Xbox 360
Controller"**: the silently-started Steam also acted on the Guide press. That is the
"PS button fights Big Picture" report, reproduced and photographed without a human.
