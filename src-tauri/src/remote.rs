// OmniDeck — phone-as-remote: a tiny authed LAN HTTP server (backlog parking-lot item).
//
// A phone on the same network gets a thumb-friendly web page (served at `/`) with transport,
// volume, and navigation buttons; every button is one authenticated HTTP call into the same
// control paths the rest of the app already uses:
//
//   transport → mpris::control (play-pause / next / previous / stop over the session bus)
//   volume    → wpctl (PipeWire), pactl fallback — spawn-and-wait like commands::power_action
//   nav       → gamepad::emit_synthetic_button: the SAME `gamepad-event` a physical pad
//               press emits, so the webview's existing input handling drives the UI with
//               zero new frontend paths. (Limitation, stated in the PR: while a launched
//               app is fullscreened in front, the webview ignores pad events — remote nav
//               steers OmniDeck's own UI, not the launched app.)
//
// SECURITY (threat model: a LAN attacker without the token gets nothing):
//   * Off by default: `[remote] enabled = false`. Nothing listens until the user opts in.
//   * A 32-byte random per-install token (hex, from /dev/urandom) is generated on first
//     enable and stored in config.toml next to media_server.token — same single-user
//     plaintext posture. It is MASKED from `get_config`/`restore_config` (commands.rs) and
//     stripped from sanitized backups (config.rs), exactly like the media-server token.
//   * Every /api request must carry the token (X-Remote-Token header or ?token= query);
//     compared in constant time (token_eq). Anything else → 401 with no detail.
//   * The token never appears in logs (request logging strips the query string).
//   * The `/` page itself is served unauthenticated — it is static HTML with no secrets;
//     the phone learns the token out-of-band via the pairing URL's #fragment (fragments
//     never leave the browser), shown in the app by `remote_status` (QR-able).
//   * Hand-rolled HTTP over std::net — no new dependency. Headers are capped at 8 KiB,
//     reads time out at 5 s, and concurrent connections are capped (excess → 503), so a
//     LAN attacker can't pin threads or buffer memory.
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// `[remote]` in config.toml. Off by default; the token is filled on first enable.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct RemoteConfig {
    /// Master switch — nothing binds a socket while false.
    pub enabled: bool,
    /// TCP port the remote listens on (all interfaces). Non-privileged only.
    pub port: u16,
    /// Per-install random token (hex). Generated on first enable; blanked over IPC the
    /// way media_server.token is. Clearing it here forces a new pairing on next enable.
    pub token: String,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self { enabled: false, port: 8765, token: String::new() }
    }
}

impl RemoteConfig {
    /// Same posture as Settings::normalize for hand-edited values: the port must be
    /// non-privileged (a hand-edited 80 would need root and silently fail to bind).
    pub fn normalize(&mut self) {
        if self.port < 1024 {
            self.port = 8765;
        }
        // A token is pasted hex from our own generator; anything with whitespace or header/
        // URL-breaking characters can't have come from it and would corrupt the HTTP compare.
        if !self.token.chars().all(|c| c.is_ascii_alphanumeric()) {
            self.token.clear();
        }
    }
}

/// Constant-time token compare. `configured` empty always rejects (a server should never
/// run without a token, but a hand-edited config must not turn into "auth disabled").
/// Length differences fold into the accumulator instead of early-returning, so timing
/// doesn't reveal how many prefix bytes matched.
pub fn token_eq(supplied: &str, configured: &str) -> bool {
    let (a, b) = (supplied.as_bytes(), configured.as_bytes());
    if b.is_empty() {
        return false;
    }
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len() {
        diff |= (a[i] ^ b[i % b.len()]) as usize;
    }
    diff == 0
}

/// 32 random bytes from /dev/urandom as 64 hex chars (Linux-only app — evdev/x11rb already
/// assume it). No rand crate needed.
pub fn generate_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .map_err(|e| format!("couldn't read /dev/urandom: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

// ---- Dispatch: what an authenticated request is allowed to do ----
//
// A trait so the HTTP plumbing is testable hermetically (tests inject a recorder; the real
// impl needs a Tauri AppHandle + session bus that unit tests don't have).

pub trait Dispatch: Send + Sync + 'static {
    fn transport(&self, action: &str) -> Result<(), String>;
    fn volume(&self, action: &str, value: Option<u8>) -> Result<(), String>;
    fn nav(&self, dir: &str) -> Result<(), String>;
    fn status(&self) -> serde_json::Value;
}

/// The real dispatcher: MPRIS for transport, wpctl/pactl for volume, synthetic
/// gamepad events for nav.
struct AppDispatch {
    app: tauri::AppHandle,
}

impl Dispatch for AppDispatch {
    fn transport(&self, action: &str) -> Result<(), String> {
        // mpris::control is async (zbus); the server runs on plain threads, so block on
        // Tauri's runtime — these are millisecond D-Bus round-trips.
        tauri::async_runtime::block_on(crate::mpris::control(action))
    }

    fn volume(&self, action: &str, value: Option<u8>) -> Result<(), String> {
        volume_command(action, value)
    }

    fn nav(&self, dir: &str) -> Result<(), String> {
        // The exact codes gilrs button presses produce (`format!("{b:?}")` in gamepad.rs),
        // so the webview can't tell a phone tap from a pad press.
        let code = match dir {
            "up" => "DPadUp",
            "down" => "DPadDown",
            "left" => "DPadLeft",
            "right" => "DPadRight",
            "select" => "South",
            "back" => "East",
            _ => return Err(format!("unknown nav direction: {dir}")),
        };
        crate::gamepad::emit_synthetic_button(&self.app, code);
        Ok(())
    }

    fn status(&self) -> serde_json::Value {
        serde_json::json!({ "ok": true, "now_playing": crate::mpris::now_playing() })
    }
}

/// System volume via wpctl (PipeWire — the modern default), falling back to pactl when
/// wpctl isn't on PATH (plain PulseAudio hosts). `.status()` waits for the exit code so a
/// failure surfaces to the phone instead of resolving Ok at fork+exec (power_action's
/// rationale). There is no existing volume path on this base to reuse — the audio-switcher
/// branch (#15) isn't part of the integration branch — so this is the whole implementation.
fn volume_command(action: &str, value: Option<u8>) -> Result<(), String> {
    let set_pct = match action {
        "set" => Some(value.ok_or("volume set needs value=0..100")?.min(100)),
        "up" | "down" | "mute" => None,
        _ => return Err(format!("unknown volume action: {action}")),
    };
    let wpctl: Vec<String> = match (action, set_pct) {
        // -l 1.0 caps "up" at 100% — repeated taps must not push past clipping.
        ("up", _) => ["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", "5%+"]
            .map(String::from)
            .to_vec(),
        ("down", _) => ["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"].map(String::from).to_vec(),
        ("mute", _) => ["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"].map(String::from).to_vec(),
        (_, Some(v)) => {
            vec!["set-volume".into(), "@DEFAULT_AUDIO_SINK@".into(), format!("{v}%")]
        }
        _ => unreachable!(),
    };
    let pactl: Vec<String> = match (action, set_pct) {
        ("up", _) => ["set-sink-volume", "@DEFAULT_SINK@", "+5%"].map(String::from).to_vec(),
        ("down", _) => ["set-sink-volume", "@DEFAULT_SINK@", "-5%"].map(String::from).to_vec(),
        ("mute", _) => ["set-sink-mute", "@DEFAULT_SINK@", "toggle"].map(String::from).to_vec(),
        (_, Some(v)) => {
            vec!["set-sink-volume".into(), "@DEFAULT_SINK@".into(), format!("{v}%")]
        }
        _ => unreachable!(),
    };
    match std::process::Command::new("wpctl").args(&wpctl).status() {
        Ok(st) if st.success() => return Ok(()),
        Ok(st) => return Err(format!("wpctl exited with {st}")),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("couldn't run wpctl: {e}"))
        }
        Err(_) => {} // no wpctl — try pactl below
    }
    match std::process::Command::new("pactl").args(&pactl).status() {
        Ok(st) if st.success() => Ok(()),
        Ok(st) => Err(format!("pactl exited with {st}")),
        Err(_) => Err("neither wpctl nor pactl is available for volume control".into()),
    }
}

// ---- HTTP plumbing ----

/// The phone page, inlined at compile time (self-contained: no CDN, inline CSS/JS).
const PAGE: &str = include_str!("remote_page.html");

/// Parsed just-enough HTTP request.
struct Request {
    method: String,
    path: String,          // path only, query stripped
    query: Vec<(String, String)>,
    token_header: Option<String>,
}

/// What the router decided; `handle_request` executes it.
#[derive(Debug, PartialEq)]
enum Route<'a> {
    Page,
    Status,
    Transport(&'a str),
    Volume(&'a str),
    Nav(&'a str),
    NotFound,
    MethodNotAllowed,
}

fn route<'a>(method: &str, path: &'a str) -> Route<'a> {
    if matches!(path, "/" | "/index.html") {
        return if method == "GET" { Route::Page } else { Route::MethodNotAllowed };
    }
    if path == "/api/status" {
        return if method == "GET" { Route::Status } else { Route::MethodNotAllowed };
    }
    if let Some(action) = path.strip_prefix("/api/transport/") {
        if matches!(action, "play-pause" | "next" | "previous" | "stop") {
            return if method == "POST" { Route::Transport(action) } else { Route::MethodNotAllowed };
        }
    }
    if let Some(action) = path.strip_prefix("/api/volume/") {
        if matches!(action, "up" | "down" | "set" | "mute") {
            return if method == "POST" { Route::Volume(action) } else { Route::MethodNotAllowed };
        }
    }
    if let Some(dir) = path.strip_prefix("/api/nav/") {
        if matches!(dir, "up" | "down" | "left" | "right" | "select" | "back") {
            return if method == "POST" { Route::Nav(dir) } else { Route::MethodNotAllowed };
        }
    }
    Route::NotFound
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: String,
}

impl Response {
    fn json(status: u16, body: serde_json::Value) -> Response {
        Response { status, content_type: "application/json", body: body.to_string() }
    }
    fn err(status: u16, msg: &str) -> Response {
        Self::json(status, serde_json::json!({ "ok": false, "error": msg }))
    }
}

/// Route + auth + execute. Pure with respect to I/O (the dispatcher is injected), so the
/// auth matrix is unit-testable without a Tauri app.
fn handle_request(req: &Request, token: &str, dispatch: &dyn Dispatch) -> Response {
    let r = route(&req.method, &req.path);
    match r {
        // The page carries no secrets; pairing happens via the URL #fragment client-side.
        Route::Page => {
            return Response { status: 200, content_type: "text/html; charset=utf-8", body: PAGE.into() }
        }
        Route::NotFound => return Response::err(404, "not found"),
        Route::MethodNotAllowed => return Response::err(405, "method not allowed"),
        _ => {}
    }
    // Everything under /api requires the token: header first, query as the fallback for
    // clients that can't set headers. Constant-time compare; failures learn nothing else.
    let supplied = req
        .token_header
        .as_deref()
        .or_else(|| req.query.iter().find(|(k, _)| k == "token").map(|(_, v)| v.as_str()))
        .unwrap_or("");
    if !token_eq(supplied, token) {
        return Response::err(401, "unauthorized");
    }
    let result = match r {
        Route::Status => return Response::json(200, dispatch.status()),
        Route::Transport(a) => dispatch.transport(a),
        Route::Volume(a) => {
            let value = req
                .query
                .iter()
                .find(|(k, _)| k == "value")
                .and_then(|(_, v)| v.parse::<u8>().ok());
            if a == "set" && value.is_none() {
                return Response::err(400, "volume set needs value=0..100");
            }
            dispatch.volume(a, value)
        }
        Route::Nav(d) => dispatch.nav(d),
        Route::Page | Route::NotFound | Route::MethodNotAllowed => unreachable!(),
    };
    match result {
        Ok(()) => Response::json(200, serde_json::json!({ "ok": true })),
        Err(e) => Response::err(500, &e),
    }
}

/// Read + parse one request off the stream. Header block capped at 8 KiB (slowloris /
/// memory guard); any parse failure is a None → the connection is dropped without a reply.
fn parse_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?).take(8 * 1024);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    if !parts.next().is_some_and(|v| v.starts_with("HTTP/1.")) {
        return None;
    }
    let (path, query_str) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q),
        None => (target.to_string(), ""),
    };
    // Tokens are hex and values numeric, so no percent-decoding is needed (documented on
    // RemoteConfig::normalize — a token that would need escaping is cleared there).
    let query: Vec<(String, String)> = query_str
        .split('&')
        .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    let mut token_header = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).ok()? == 0 {
            break; // EOF before the blank line — treat headers as done
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((name, value)) = h.split_once(':') {
            if name.eq_ignore_ascii_case("x-remote-token") {
                token_header = Some(value.trim().to_string());
            }
        }
    }
    Some(Request { method, path, query, token_header })
}

fn write_response(stream: &mut TcpStream, resp: &Response) {
    let reason = match resp.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        resp.status,
        reason,
        resp.content_type,
        resp.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(resp.body.as_bytes());
    let _ = stream.flush();
}

/// A running listener. Owned by the RUNNING slot (the commands) or a test.
pub struct Server {
    port: u16,
    shutdown: Arc<AtomicBool>,
}

impl Server {
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblock the accept() so the loop observes the flag and exits.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Concurrent-connection cap: a phone remote is one client; anything past a small burst is
/// an attacker or a bug. Excess connections get an immediate 503.
const MAX_CONNS: usize = 8;

/// Spawn the accept loop on `listener` (already bound — tests bind port 0 for an
/// ephemeral port). Never logs the token.
fn spawn_server(listener: TcpListener, token: String, dispatch: Arc<dyn Dispatch>) -> Server {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let shutdown = Arc::new(AtomicBool::new(false));
    let active = Arc::new(AtomicUsize::new(0));
    let flag = shutdown.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            let Ok(mut stream) = stream else { continue };
            if active.load(Ordering::SeqCst) >= MAX_CONNS {
                write_response(&mut stream, &Response::err(503, "too many connections"));
                continue;
            }
            active.fetch_add(1, Ordering::SeqCst);
            let dispatch = dispatch.clone();
            let token = token.clone();
            let active = active.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                if let Some(req) = parse_request(&stream) {
                    // Query string deliberately not logged — it may carry the token.
                    tracing::debug!("remote: {} {}", req.method, req.path);
                    let resp = handle_request(&req, &token, dispatch.as_ref());
                    write_response(&mut stream, &resp);
                }
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
        tracing::info!("remote: listener on port {port} stopped");
    });
    Server { port, shutdown }
}

/// The one live server behind the Tauri commands (tests construct Servers directly and
/// never touch this slot, so `cargo test`'s parallel threads don't fight over it).
static RUNNING: Mutex<Option<Server>> = Mutex::new(None);

fn start(port: u16, token: String, dispatch: Arc<dyn Dispatch>) -> Result<u16, String> {
    let mut slot = crate::sync::lock_or_recover(&RUNNING, "remote.server");
    if let Some(old) = slot.take() {
        old.stop();
    }
    let listener = TcpListener::bind(("0.0.0.0", port))
        .map_err(|e| format!("remote: couldn't listen on port {port}: {e}"))?;
    let server = spawn_server(listener, token, dispatch);
    let bound = server.port;
    *slot = Some(server);
    tracing::info!("remote: phone remote listening on 0.0.0.0:{bound}");
    Ok(bound)
}

fn stop() {
    if let Some(server) = crate::sync::lock_or_recover(&RUNNING, "remote.server").take() {
        server.stop();
    }
}

/// Boot-time start when `[remote] enabled = true` (lib.rs setup). A hand-enabled config
/// with no token gets one generated + saved, same as first enable through the UI.
pub fn spawn_if_enabled(app: tauri::AppHandle) {
    let rc = crate::config::load_or_create().remote;
    if !rc.enabled {
        return;
    }
    let token = if rc.token.is_empty() {
        let t = match generate_token() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("remote: token generation failed ({e}) — remote NOT started");
                return;
            }
        };
        let saved = t.clone();
        if let Err(e) = crate::config::update_remote(move |r| r.token = saved) {
            tracing::warn!("remote: couldn't persist token ({e}) — remote NOT started");
            return;
        }
        t
    } else {
        rc.token
    };
    if let Err(e) = start(rc.port, token, Arc::new(AppDispatch { app })) {
        tracing::warn!("{e}");
    }
}

/// What the settings UI needs: enabled/port/running plus the QR-able pairing URL. The URL
/// carries the token in the #fragment ON PURPOSE — this command IS the pairing surface
/// (the QR the user scans); fragments are never sent over the network by browsers. It is
/// the one deliberate exception to "the webview never sees the token", mirroring how the
/// PIN of a pairing dialog must be displayable to be usable.
#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RemoteStatus {
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    /// `http://<lan-ip>:<port>/#<token>` when enabled and paired; None otherwise.
    pub url: Option<String>,
}

/// Best-effort LAN address: a connected UDP socket picks the outbound interface without
/// sending a packet. None (e.g. no route) just means the UI shows the port instead of a URL.
fn lan_ip() -> Option<std::net::IpAddr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect(("1.1.1.1", 80)).ok()?;
    Some(sock.local_addr().ok()?.ip())
}

fn current_status() -> RemoteStatus {
    let rc = crate::config::load_or_create().remote;
    let running = crate::sync::lock_or_recover(&RUNNING, "remote.server").is_some();
    let url = (rc.enabled && !rc.token.is_empty())
        .then(|| lan_ip().map(|ip| format!("http://{ip}:{}/#{}", rc.port, rc.token)))
        .flatten();
    RemoteStatus { enabled: rc.enabled, running, port: rc.port, url }
}

#[tauri::command]
pub fn remote_status() -> RemoteStatus {
    current_status()
}

/// Enable: generate + persist a token on first use, save `enabled`, start the listener.
/// Disable: save + stop. Errors (e.g. port already in use) surface for the UI to toast.
#[tauri::command]
pub fn set_remote_enabled(app: tauri::AppHandle, enabled: bool) -> Result<RemoteStatus, String> {
    if enabled {
        let mut rc = crate::config::load_or_create().remote;
        if rc.token.is_empty() {
            rc.token = generate_token()?;
        }
        rc.enabled = true;
        let saved = rc.clone();
        crate::config::update_remote(move |r| *r = saved)?;
        start(rc.port, rc.token, Arc::new(AppDispatch { app }))?;
    } else {
        crate::config::update_remote(|r| r.enabled = false)?;
        stop();
    }
    Ok(current_status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_eq_is_strict() {
        assert!(token_eq("abc123", "abc123"));
        assert!(!token_eq("abc124", "abc123"));
        assert!(!token_eq("abc12", "abc123")); // prefix
        assert!(!token_eq("abc1234", "abc123")); // longer
        assert!(!token_eq("", "abc123"));
        // An empty CONFIGURED token rejects everything — never "auth disabled".
        assert!(!token_eq("", ""));
        assert!(!token_eq("anything", ""));
    }

    #[test]
    fn generated_tokens_are_long_random_hex() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn normalize_rejects_privileged_ports_and_broken_tokens() {
        let mut rc = RemoteConfig { port: 80, ..Default::default() };
        rc.normalize();
        assert_eq!(rc.port, 8765);
        let mut rc = RemoteConfig { token: "has space\r\nInjected: yes".into(), ..Default::default() };
        rc.normalize();
        assert_eq!(rc.token, "");
        let mut rc = RemoteConfig { port: 9000, token: "abc123DEF".into(), enabled: true };
        rc.normalize();
        assert_eq!((rc.port, rc.token.as_str(), rc.enabled), (9000, "abc123DEF", true));
    }

    #[test]
    fn router_maps_paths_and_methods() {
        assert_eq!(route("GET", "/"), Route::Page);
        assert_eq!(route("GET", "/api/status"), Route::Status);
        assert_eq!(route("POST", "/api/transport/play-pause"), Route::Transport("play-pause"));
        assert_eq!(route("POST", "/api/transport/stop"), Route::Transport("stop"));
        assert_eq!(route("POST", "/api/volume/up"), Route::Volume("up"));
        assert_eq!(route("POST", "/api/nav/select"), Route::Nav("select"));
        // Wrong method on a real path is 405, unknown paths/actions are 404.
        assert_eq!(route("GET", "/api/transport/next"), Route::MethodNotAllowed);
        assert_eq!(route("POST", "/"), Route::MethodNotAllowed);
        assert_eq!(route("POST", "/api/transport/eject"), Route::NotFound);
        assert_eq!(route("GET", "/etc/passwd"), Route::NotFound);
    }

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
    }

    impl Dispatch for Recorder {
        fn transport(&self, action: &str) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("transport:{action}"));
            Ok(())
        }
        fn volume(&self, action: &str, value: Option<u8>) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("volume:{action}:{value:?}"));
            Ok(())
        }
        fn nav(&self, dir: &str) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("nav:{dir}"));
            Ok(())
        }
        fn status(&self) -> serde_json::Value {
            serde_json::json!({ "ok": true, "test": true })
        }
    }

    /// One raw round-trip against a live loopback listener (hermetic — the artcache-PR
    /// test pattern: bind port 0, talk real HTTP over a TcpStream, assert on the bytes).
    fn send(port: u16, raw: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        s.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    #[test]
    fn loopback_auth_matrix_and_routing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let recorder = Arc::new(Recorder::default());
        let server = spawn_server(listener, "sekrit-token".into(), recorder.clone());

        // The page is served without auth and carries no token.
        let page = send(port, "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("OmniDeck Remote"), "page body expected");
        assert!(!page.contains("sekrit-token"), "page must not leak the token");

        // No token → 401 and the dispatcher is NEVER reached.
        let r = send(port, "POST /api/transport/play-pause HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 401"), "{r}");
        // Wrong token → 401.
        let r = send(
            port,
            "POST /api/transport/play-pause HTTP/1.1\r\nHost: x\r\nX-Remote-Token: wrong\r\n\r\n",
        );
        assert!(r.starts_with("HTTP/1.1 401"), "{r}");
        assert!(recorder.calls.lock().unwrap().is_empty(), "unauthorized must not dispatch");

        // Right token in the header → dispatched.
        let r = send(
            port,
            "POST /api/transport/play-pause HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n",
        );
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        // Right token as a query param (header-less client fallback) → dispatched.
        let r = send(port, "POST /api/nav/select?token=sekrit-token HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        // Volume set carries its value; without one it's a 400.
        let r = send(
            port,
            "POST /api/volume/set?value=40 HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n",
        );
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        let r = send(
            port,
            "POST /api/volume/set HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n",
        );
        assert!(r.starts_with("HTTP/1.1 400"), "{r}");
        // Authenticated status GET.
        let r = send(
            port,
            "GET /api/status HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n",
        );
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        assert!(r.contains("\"test\":true"), "{r}");
        // Unknown path (authed or not) → 404; wrong method → 405.
        let r = send(port, "POST /api/other HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 404"), "{r}");
        let r = send(port, "GET /api/transport/next HTTP/1.1\r\nHost: x\r\nX-Remote-Token: sekrit-token\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 405"), "{r}");

        assert_eq!(
            *recorder.calls.lock().unwrap(),
            vec![
                "transport:play-pause".to_string(),
                "nav:select".to_string(),
                "volume:set:Some(40)".to_string(),
            ]
        );
        server.stop();
    }

    #[test]
    fn stopped_server_releases_the_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = spawn_server(listener, "t".into(), Arc::new(Recorder::default()));
        server.stop();
        // The accept loop exits and the port becomes bindable again (poll briefly — the
        // loop needs a beat to observe the flag).
        for _ in 0..50 {
            if TcpListener::bind(("127.0.0.1", port)).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("port {port} still bound after stop()");
    }
}
