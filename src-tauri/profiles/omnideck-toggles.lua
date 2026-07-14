-- # omnideck-generated v1 — per-dimension playback toggles (replaces preset profiles).
-- Delete this first line to take ownership (OmniDeck stops rewriting the file).
-- {{TIER_INFO}}
--
-- One key per DIMENSION instead of one profile per combination — toggles compose:
--   F1  reset       everything off / back to defaults
--   F2  stretch     fill the screen (panscan) on/off
--   F3  denoise     KNLMeansCL on/off (composes with interpolation)
--   F4  interpolate off -> smooth (full display rate) -> ultra (optical flow) -> off
--   F5  upscaling   high (ewa_lanczossharp) -> regular (spline36) -> off (bilinear)
--   F6  tone smooth deband (the 8-bit -> 10-bit gradient reconstruction) on/off
--   F9  status      show the current state of everything
--
-- Also heals the ultra seek-desync (2026-07-10 couch find): seeking flushes the
-- optical-flow filter's timing state and it can come back misaligned WITHOUT the A-V
-- counter noticing. On every seek while interpolation is active, the filter chain is
-- dropped and re-applied once playback restarts — the same rebuild that switching
-- profiles by hand used to do, now automatic and instant.

local mp = require "mp"

local VPY = "{{PROFILE_DIR}}"

-- ---- state ----------------------------------------------------------------
local interp = 1 -- index into INTERP
local INTERP = {
  { name = "off",    file = nil },
  { name = "smooth", file = VPY .. "/interpolate-basic.vpy" },
  { name = "ultra",  file = VPY .. "/interpolate-ultra.vpy" },
}
local denoise = false
local stretch = false
local UPSCALE = {
  { name = "high",    scale = "ewa_lanczossharp" },
  { name = "regular", scale = "spline36" },
  { name = "off",     scale = "bilinear" },
}
local upscale = 1
-- Set true by a 'seek' while interpolation is active; consumed on the next 'playback-restart'
-- to rebuild the flushed optical-flow filter. Declared here (before rebuild_vf) so a manual
-- rebuild during the seek→restart window cancels a now-redundant heal (no double @omniinterp).
local heal_pending = false

-- ---- vf chain -------------------------------------------------------------
-- Labeled entries so each dimension owns its slot; rebuild keeps a fixed order
-- (denoise feeds interpolation — denoising retimed frames would cost 2-7x more).
local function rebuild_vf()
  mp.commandv("vf", "remove", "@omnidn")
  mp.commandv("vf", "remove", "@omniinterp")
  if denoise then
    mp.commandv("vf", "append", "@omnidn:vapoursynth=" .. VPY .. "/denoise.vpy")
  end
  local f = INTERP[interp].file
  if f then
    mp.commandv("vf", "append", "@omniinterp:vapoursynth=" .. f)
  end
  heal_pending = false -- rebuild re-established @omniinterp; cancel any pending seek-heal
end

local function osd(msg, secs) mp.osd_message(msg, secs or 1.6) end

-- ---- toggles ---------------------------------------------------------------
local function cycle_interp()
  interp = interp % #INTERP + 1
  rebuild_vf()
  local extra = ({ [2] = " (full display rate)", [3] = " (display/2 — seek-safe)" })[interp] or ""
  osd("Interpolation: " .. INTERP[interp].name:upper() .. extra)
end

local function toggle_denoise()
  denoise = not denoise
  rebuild_vf()
  osd("Denoise: " .. (denoise and "ON" or "OFF"))
end

local function toggle_stretch()
  stretch = not stretch
  mp.set_property_number("panscan", stretch and 1.0 or 0.0)
  osd("Stretch to fill: " .. (stretch and "ON" or "OFF"))
end

local function cycle_upscale()
  upscale = upscale % #UPSCALE + 1
  mp.set_property("scale", UPSCALE[upscale].scale)
  osd("Upscaling: " .. UPSCALE[upscale].name:upper() .. " (" .. UPSCALE[upscale].scale .. ")")
end

local function toggle_deband()
  local v = not mp.get_property_bool("deband")
  mp.set_property_bool("deband", v)
  osd("Tone smoothing (deband): " .. (v and "ON" or "OFF"))
end

local function reset_all()
  interp, denoise, stretch, upscale = 1, false, false, 1
  rebuild_vf()
  mp.set_property_number("panscan", 0.0)
  mp.set_property("scale", UPSCALE[1].scale)
  mp.set_property_bool("deband", true) -- generated mpv.conf default
  osd("Playback filters: reset (all off, defaults restored)")
end

local function show_status()
  osd(string.format(
    "interp %s · upscale %s · denoise %s · deband %s · stretch %s",
    INTERP[interp].name, UPSCALE[upscale].name,
    denoise and "on" or "off",
    mp.get_property_bool("deband") and "on" or "off",
    stretch and "on" or "off"), 3)
end

-- ---- seek self-heal ---------------------------------------------------------
-- Drop the interpolation entry the moment a seek starts; re-append when playback
-- restarts. Cheap when interpolation is off (no-op), invisible when on (~1 frame).
-- heal_pending is declared up in the state section so rebuild_vf can cancel it.
mp.register_event("seek", function()
  if INTERP[interp].file then
    mp.commandv("vf", "remove", "@omniinterp")
    heal_pending = true
  end
end)
mp.register_event("playback-restart", function()
  if heal_pending then
    heal_pending = false
    local f = INTERP[interp].file
    if f then mp.commandv("vf", "append", "@omniinterp:vapoursynth=" .. f) end
  end
end)

-- ---- bindings ---------------------------------------------------------------
mp.add_key_binding("F1", "omnideck-reset", reset_all)
mp.add_key_binding("F2", "omnideck-stretch", toggle_stretch)
mp.add_key_binding("F3", "omnideck-denoise", toggle_denoise)
mp.add_key_binding("F4", "omnideck-interp", cycle_interp)
mp.add_key_binding("F5", "omnideck-upscale", cycle_upscale)
mp.add_key_binding("F6", "omnideck-deband", toggle_deband)
mp.add_key_binding("F9", "omnideck-status", show_status)
