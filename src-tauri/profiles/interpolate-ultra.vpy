# omnideck-generated v1 — ultra motion interpolation (MVTools FlowFPS: optical flow).
# Delete this first line to take ownership (OmniDeck stops rewriting the file).
# {{TIER_INFO}}
import os
import vapoursynth as vs
core = vs.core
clip = video_in

# Same display-rate resolution as interpolate-basic.vpy (see the comment there):
# env override > mpv-injected display_fps > OmniDeck's baked session rate.
try:
    dfps = float(os.environ.get("OMNIDECK_DISPLAY_FPS", "0"))
except ValueError:
    dfps = 0.0
if not dfps > 30:
    dfps = display_fps if display_fps and display_fps > 30 else float("{{DISPLAY_FPS_HINT}}")

if container_fps and container_fps > 1:
    clip = core.std.AssumeFPS(clip, fpsnum=int(round(container_fps * 1000)), fpsden=1000)
else:
    clip = core.std.AssumeFPS(clip, fpsnum=24000, fpsden=1001)

w, h = clip.width // 4 * 4, clip.height // 4 * 4
if (w, h) != (clip.width, clip.height):
    clip = core.resize.Bicubic(clip, width=w, height=h)

# FlowFPS (optical flow) is far heavier than BlockFPS: at 165 fps / 1440p it cannot keep
# up on the CPU for long movies (measured on a 14700K) — the filter falls behind and
# audio slowly walks away from video (felt as desync, invisible in the stats tab because
# mpv's clock looks fine). Cap at half the display rate on high-refresh panels: display/2
# is still glass-smooth and sustainable. The BASIC profile keeps the full display target.
if dfps > 30:
    t = dfps / 2 if dfps > 100 else dfps
    target_num, target_den = int(round(t * 1000)), 1000
else:
    target_num, target_den = 60, 1

src_fps = clip.fps.numerator / clip.fps.denominator if clip.fps.denominator else 0
if src_fps > 59:
    clip.set_output()
else:
    sup = core.mv.Super(clip, pel=2, sharp=2)
    bv = core.mv.Analyse(sup, isb=True, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    fv = core.mv.Analyse(sup, isb=False, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    core.mv.FlowFPS(clip, sup, bv, fv, num=target_num, den=target_den,
                    thscd1=400, thscd2=130).set_output()
