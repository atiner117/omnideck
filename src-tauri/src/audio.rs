// OmniDeck — audio output (sink) switching via pactl (PipeWire-Pulse or PulseAudio).
// Enumeration prefers `pactl -f json list sinks`; older pactl without JSON support falls
// back to `pactl list short sinks`. All invocations are argv-only (no shell), and
// `audio_set_output` only accepts a sink name that came from our own enumeration, so a
// crafted frontend value can never reach pactl.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AudioSink {
    /// Internal sink name (e.g. `alsa_output.pci-0000_0b_00.4.analog-stereo`) — the id
    /// passed back to `audio_set_output`.
    pub name: String,
    /// Human-readable label (e.g. "Built-in Audio Analog Stereo").
    pub description: String,
    pub is_default: bool,
}

fn pactl(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("pactl")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run pactl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`pactl {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parse `pactl -f json list sinks` output. None if the payload isn't the expected
/// JSON array (e.g. a pactl too old for `-f json` that printed something else).
fn parse_sinks_json(json: &str, default_sink: &str) -> Option<Vec<AudioSink>> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let sinks = v
        .as_array()?
        .iter()
        .filter_map(|s| {
            let name = s.get("name")?.as_str()?.to_string();
            let description = s
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or(&name)
                .to_string();
            let is_default = name == default_sink;
            Some(AudioSink { name, description, is_default })
        })
        .collect();
    Some(sinks)
}

/// Parse `pactl list short sinks` (tab-separated: index, name, driver, sample spec,
/// state). No description column in this format, so the name doubles as the label.
fn parse_sinks_short(text: &str, default_sink: &str) -> Vec<AudioSink> {
    text.lines()
        .filter_map(|line| {
            let name = line.split('\t').nth(1)?.trim();
            if name.is_empty() {
                return None;
            }
            Some(AudioSink {
                name: name.to_string(),
                description: name.to_string(),
                is_default: name == default_sink,
            })
        })
        .collect()
}

fn list_sinks() -> Result<Vec<AudioSink>, String> {
    // Best-effort: without a default sink (headless pulse, race at startup) the list is
    // still useful, just with no row marked default.
    let default_sink =
        pactl(&["get-default-sink"]).map(|s| s.trim().to_string()).unwrap_or_default();
    if let Ok(json) = pactl(&["-f", "json", "list", "sinks"]) {
        if let Some(sinks) = parse_sinks_json(&json, &default_sink) {
            return Ok(sinks);
        }
    }
    Ok(parse_sinks_short(&pactl(&["list", "short", "sinks"])?, &default_sink))
}

fn validate_sink_id<'a>(sinks: &'a [AudioSink], id: &str) -> Result<&'a AudioSink, String> {
    sinks
        .iter()
        .find(|s| s.name == id)
        .ok_or_else(|| format!("unknown audio sink: {id}"))
}

#[tauri::command]
pub fn audio_outputs() -> Result<Vec<AudioSink>, String> {
    list_sinks()
}

#[tauri::command]
pub fn audio_set_output(id: String) -> Result<(), String> {
    let sinks = list_sinks()?;
    let sink = validate_sink_id(&sinks, &id)?;
    let status = std::process::Command::new("pactl")
        .args(["set-default-sink", &sink.name])
        .status()
        .map_err(|e| format!("failed to run pactl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
        Err(format!("`pactl set-default-sink {}` failed (exit {code})", sink.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_SINKS: &str = r#"[
        {"index": 46, "name": "alsa_output.pci-0000_0b_00.4.analog-stereo",
         "description": "Starship/Matisse HD Audio Controller Analog Stereo",
         "state": "RUNNING"},
        {"index": 51, "name": "alsa_output.pci-0000_09_00.1.hdmi-stereo",
         "description": "TU104 HD Audio Controller Digital Stereo (HDMI)",
         "state": "SUSPENDED"}
    ]"#;

    #[test]
    fn parses_json_sinks_and_marks_default() {
        let sinks =
            parse_sinks_json(JSON_SINKS, "alsa_output.pci-0000_09_00.1.hdmi-stereo").unwrap();
        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[0].name, "alsa_output.pci-0000_0b_00.4.analog-stereo");
        assert_eq!(sinks[0].description, "Starship/Matisse HD Audio Controller Analog Stereo");
        assert!(!sinks[0].is_default);
        assert!(sinks[1].is_default);
    }

    #[test]
    fn json_parse_rejects_non_array_output() {
        // A pactl without -f json prints usage/garbage — the caller must fall back.
        assert!(parse_sinks_json("Usage: pactl ...", "x").is_none());
        assert!(parse_sinks_json("{\"not\": \"an array\"}", "x").is_none());
    }

    #[test]
    fn parses_short_sinks() {
        let short = "46\talsa_output.pci-0000_0b_00.4.analog-stereo\tPipeWire\ts32le 2ch 48000Hz\tRUNNING\n\
                     51\talsa_output.pci-0000_09_00.1.hdmi-stereo\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n";
        let sinks = parse_sinks_short(short, "alsa_output.pci-0000_0b_00.4.analog-stereo");
        assert_eq!(sinks.len(), 2);
        assert!(sinks[0].is_default);
        // Short format has no description column — the name stands in.
        assert_eq!(sinks[1].description, "alsa_output.pci-0000_09_00.1.hdmi-stereo");
        assert!(!sinks[1].is_default);
    }

    #[test]
    fn set_output_rejects_unknown_sink_id() {
        let sinks = parse_sinks_json(JSON_SINKS, "").unwrap();
        assert!(validate_sink_id(&sinks, "alsa_output.pci-0000_0b_00.4.analog-stereo").is_ok());
        assert!(validate_sink_id(&sinks, "evil; rm -rf /").is_err());
        assert!(validate_sink_id(&sinks, "").is_err());
    }
}
