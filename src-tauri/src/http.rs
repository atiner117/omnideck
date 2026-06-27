// OmniDeck — shared HTTP client.
// One reqwest client, reused everywhere we fetch (SteamGridDB art, site favicons). It carries
// the timeout policy so a slow/hung CDN edge or a captive portal — one that accepts the TCP
// connection but never sends or closes — can't wedge an async Tauri command (`grid_art`/
// `app_icon` stay inflight forever, pinning the FE spinner) or freeze the whole process on the
// `--gridart` CLI path (`block_on`). Reusing one client also pools connections.
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// The shared client. Timeouts: connect 5s (TCP + TLS handshake), read 10s (inactivity between
/// body reads — this is what kills the streaming `.chunk()` hang when a server accepts the
/// connection but never sends), total 15s (whole-request budget, incl. `.json()`).
///
/// Built once; if the builder ever fails (it won't with the rustls backend) we fall back to a
/// default client so callers always get something usable rather than panicking.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    })
}
