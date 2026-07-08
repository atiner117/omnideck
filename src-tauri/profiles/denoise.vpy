# omnideck-generated v1 — denoise via KNLMeansCL (OpenCL — runs on the GPU, very
# effective on grain). Delete this first line to take ownership.
# {{TIER_INFO}}
import vapoursynth as vs
core = vs.core
clip = video_in

# The system plugin exists but doesn't always autoload; try the common distro paths.
if not hasattr(core, "knlm"):
    for p in ("/usr/lib/vapoursynth/libknlmeanscl.so",
              "/usr/lib/x86_64-linux-gnu/vapoursynth/libknlmeanscl.so",
              "/usr/lib64/vapoursynth/libknlmeanscl.so"):
        try:
            core.std.LoadPlugin(p)
            break
        except vs.Error:
            pass

# No mod-4 crop here: unlike MVTools, KNLMeansCL has no block-alignment requirement, so
# cropping would only resample away edge pixels for no benefit. Denoise keeps the source
# geometry and rate untouched (KNLMeansCL never retimes).
if hasattr(core, "knlm"):
    # h controls the strength; 1.2 is gentle. d=2 uses two temporal neighbours each way.
    core.knlm.KNLMeansCL(clip, d=2, a=2, h=1.2).set_output()
else:
    # Plugin unavailable — pass through rather than kill playback.
    clip.set_output()
