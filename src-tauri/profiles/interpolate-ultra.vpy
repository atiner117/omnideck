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

# Sustainability cap #2 (2026-07-08, two-host bench): FlowFPS cost scales with SOURCE
# pixels × TARGET rate, and a 60 Hz display never trips the halving above — yet a 4K
# source targeting 60 measured 13.5 of 16 cores on a 7800X3D with EASY synthetic motion;
# real film costs a multiple of that and desyncs. Budget = cpu_threads × 12 Mpx/s (baked
# by OmniDeck), anchored to ares (14700K/28t = 336 Mpx/s): its known-good 1080p→82.5
# fits (171M), its known-desync 1080p→165 doesn't (342M). Over budget: lower the target
# to fit. 0 = CPU thread count unknown, cap disabled.
budget = float("{{ULTRA_BUDGET_PX}}")
target = target_num / target_den
if budget > 0 and w * h * target > budget:
    target = budget / (w * h)
    target_num, target_den = int(round(target * 1000)), 1000

src_fps = clip.fps.numerator / clip.fps.denominator if clip.fps.denominator else 0
# Interpolate only when it at least DOUBLES the source rate — a capped target below that
# buys judder, not smoothness (a 4K source on a modest CPU lands here; F4 basic still
# offers the full display target, and native 24 looks better than starved 40).
if src_fps > 59 or target < 2 * src_fps:
    clip.set_output()
else:
    sup = core.mv.Super(clip, pel=2, sharp=2)
    bv = core.mv.Analyse(sup, isb=True, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    fv = core.mv.Analyse(sup, isb=False, delta=1, blksize=16, overlap=8, search=3, searchparam=4)
    core.mv.FlowFPS(clip, sup, bv, fv, num=target_num, den=target_den,
                    thscd1=400, thscd2=130).set_output()
