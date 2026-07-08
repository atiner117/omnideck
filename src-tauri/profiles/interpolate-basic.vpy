# omnideck-generated v1 — basic motion interpolation (MVTools BlockFPS: cheap, robust).
# Delete this first line to take ownership (OmniDeck stops rewriting the file).
# {{TIER_INFO}}
#
# Rate-aware, bit-depth-preserving, display-aware.
import os
import vapoursynth as vs
core = vs.core
clip = video_in

# Display rate, most-authoritative first: OMNIDECK_DISPLAY_FPS env (explicit override,
# also the test hook) > mpv's injected display_fps > the rate OmniDeck baked from the
# session's RandR mode. mpv usually can't inject display_fps at filter init (the VO
# isn't up yet, and --display-fps-override is NOT forwarded here — measured, mpv 0.40)
# and passes 0 — the baked hint is what makes a 165 Hz panel actually get 165 instead
# of the 60 fallback.
try:
    dfps = float(os.environ.get("OMNIDECK_DISPLAY_FPS", "0"))
except ValueError:
    dfps = 0.0
if not dfps > 30:
    dfps = display_fps if display_fps and display_fps > 30 else float("{{DISPLAY_FPS_HINT}}")

# mpv injects the real source rate (container_fps); AssumeFPS is only the fallback for
# streams that don't declare one. MVTools needs a constant declared rate to retime from.
if container_fps and container_fps > 1:
    clip = core.std.AssumeFPS(clip, fpsnum=int(round(container_fps * 1000)), fpsden=1000)
else:
    clip = core.std.AssumeFPS(clip, fpsnum=24000, fpsden=1001)

# mod-4 crop for MVTools block alignment — WITHOUT forcing YUV420P8 (10-bit stays
# 10-bit; MVTools handles high bit depth natively).
w, h = clip.width // 4 * 4, clip.height // 4 * 4
if (w, h) != (clip.width, clip.height):
    clip = core.resize.Bicubic(clip, width=w, height=h)

# Target the panel mpv is actually driving; fall back to 60 when nothing knows it.
target_num, target_den = (int(round(dfps * 1000)), 1000) if dfps > 30 else (60, 1)

src_fps = clip.fps.numerator / clip.fps.denominator if clip.fps.denominator else 0
if src_fps > 59:
    # Already high-rate — don't double-interpolate (mpv.conf has the same guard).
    clip.set_output()
else:
    sup = core.mv.Super(clip, pel=2, sharp=2)
    bv = core.mv.Analyse(sup, isb=True, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    fv = core.mv.Analyse(sup, isb=False, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    core.mv.BlockFPS(clip, sup, bv, fv, num=target_num, den=target_den,
                     mode=3, thscd1=400, thscd2=130).set_output()
