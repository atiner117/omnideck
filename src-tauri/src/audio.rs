// OmniDeck — audio output (sink) switching via pactl (PipeWire-Pulse or PulseAudio).
// Enumeration prefers `pactl -f json list sinks`; older pactl without JSON support falls
// back to `pactl list short sinks`. All invocations are argv-only (no shell), and
// `audio_set_output` only accepts a sink name that came from our own enumeration, so a
// crafted frontend value can never reach pactl.
//
// Every pactl invocation is bounded by `PACTL_TIMEOUT` (a wedged sound-server socket
// otherwise hangs the child forever), and the commands are async + spawn_blocking so
// even the bounded wait never runs on the UI/IPC thread.

use serde::Serialize;
use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AudioSink {
    /// Internal sink name (e.g. `alsa_output.pci-0000_0b_00.4.analog-stereo`) — the id
    /// passed back to `audio_set_output`.
    pub name: String,
    /// Human-readable label (e.g. "Built-in Audio Analog Stereo").
    pub description: String,
    pub is_default: bool,
}

/// Hard cap on any single pactl invocation. pactl talks to the local PipeWire/Pulse
/// socket and normally answers in milliseconds; if the sound server wedges, an unbounded
/// `.output()`/`.status()` wait would hang the IPC call forever.
const PACTL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
struct CmdOutput {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run `cmd args…` with captured output, killing it after `timeout`. The pipes are
/// drained on their own threads so a chatty child can't fill the pipe buffer and
/// deadlock against the wait loop; the poll interval only adds ~20 ms of latency.
fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Result<CmdOutput, String> {
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {cmd}: {e}"))?;

    fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    }
    let out_thread = drain(child.stdout.take());
    let err_thread = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait(); // reap; closes the pipes so the drain threads finish
                return Err(format!(
                    "`{cmd} {}` timed out after {}s — sound server not responding?",
                    args.join(" "),
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("failed waiting for {cmd}: {e}"));
            }
        }
    };
    let stdout = String::from_utf8_lossy(&out_thread.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_thread.join().unwrap_or_default()).into_owned();
    Ok(CmdOutput { success: status.success(), code: status.code(), stdout, stderr })
}

fn pactl(args: &[&str]) -> Result<String, String> {
    let out = run_with_timeout("pactl", args, PACTL_TIMEOUT)?;
    if !out.success {
        return Err(format!("`pactl {}` failed: {}", args.join(" "), out.stderr.trim()));
    }
    Ok(out.stdout)
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

fn set_output(id: &str) -> Result<(), String> {
    let sinks = list_sinks()?;
    let sink = validate_sink_id(&sinks, id)?;
    let out = run_with_timeout("pactl", &["set-default-sink", &sink.name], PACTL_TIMEOUT)?;
    if out.success {
        Ok(())
    } else {
        let code = out.code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
        Err(format!(
            "`pactl set-default-sink {}` failed (exit {code}): {}",
            sink.name,
            out.stderr.trim()
        ))
    }
}

#[tauri::command]
pub async fn audio_outputs() -> Result<Vec<AudioSink>, String> {
    // Blocking pool: pactl is fast when healthy, but even the bounded 3 s worst case
    // must not run on the IPC thread.
    tauri::async_runtime::spawn_blocking(list_sinks)
        .await
        .map_err(|e| format!("audio task failed: {e}"))?
}

#[tauri::command]
pub async fn audio_set_output(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_output(&id))
        .await
        .map_err(|e| format!("audio task failed: {e}"))?
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

    #[test]
    fn run_with_timeout_captures_output() {
        let out = run_with_timeout("echo", &["hello"], Duration::from_secs(5)).unwrap();
        assert!(out.success);
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    fn run_with_timeout_kills_a_hung_child() {
        let started = Instant::now();
        let err = run_with_timeout("sleep", &["30"], Duration::from_millis(150)).unwrap_err();
        assert!(err.contains("timed out"), "got: {err}");
        // Killed promptly — nowhere near the child's 30 s, and no zombie left waiting.
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn run_with_timeout_reports_spawn_failure() {
        assert!(run_with_timeout("omnideck-no-such-binary", &[], Duration::from_secs(1)).is_err());
    }
}
