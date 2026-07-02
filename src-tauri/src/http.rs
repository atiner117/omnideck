// OmniDeck — shared HTTP client.
// One reqwest client, reused everywhere we fetch (SteamGridDB art, site favicons). It carries
// the timeout policy so a slow/hung CDN edge or a captive portal — one that accepts the TCP
// connection but never sends or closes — can't wedge an async Tauri command (`grid_art`/
// `app_icon` stay inflight forever, pinning the FE spinner) or freeze the whole process on the
// `gridart` CLI path (`block_on`). Reusing one client also pools connections.
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// True if `host` (possibly `ip:port`) is a loopback/private/link-local address we must not
/// probe — a crafted/imported config tile URL like `http://169.254.169.254` would otherwise
/// turn an icon/art fetch into an internal-service probe (SSRF). Checked at the fetch entry
/// points (icons::favicon) AND on every redirect hop (the client's policy below), so a public
/// host can't 302 us into the internal network. Only literal IPs are caught (single-user
/// import threat model; `icons::domain_of` already drops dot-less hosts like `localhost`).
pub fn is_blocked_host(host: &str) -> bool {
    let h = host.rsplit_once(':').map(|(a, _)| a).unwrap_or(host); // strip :port
    let h = h.strip_suffix('.').unwrap_or(h); // "127.0.0.1." resolves like "127.0.0.1"
    if let Ok(ip) = h.parse::<std::net::Ipv4Addr>() {
        return ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_unspecified()
            || ip.is_broadcast()
            || ip.octets()[0] == 0;
    }
    // Not a strict dotted-quad but still all-numeric (e.g. "127.1", "0x7f.0.0.1"): the system
    // resolver (inet_aton) accepts these short/hex forms, so block anything numeric we couldn't
    // verify as a public address. Real domains always end in an alphabetic TLD.
    let numericish = h.split('.').all(|l| {
        !l.is_empty()
            && (l.bytes().all(|b| b.is_ascii_digit())
                || (l.len() > 2
                    && (l.starts_with("0x") || l.starts_with("0X"))
                    && l.as_bytes()[2..].iter().all(u8::is_ascii_hexdigit)))
    });
    if numericish {
        return true;
    }
    matches!(h, "localhost" | "localhost.localdomain")
}

/// The shared client. Timeouts: connect 5s (TCP + TLS handshake), read 10s (inactivity between
/// body reads — this is what kills the streaming `.chunk()` hang when a server accepts the
/// connection but never sends), total 15s (whole-request budget, incl. `.json()`).
///
/// Built once; if the builder ever fails (it won't with the rustls backend) we fall back to a
/// default client so callers always get something usable rather than panicking.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            // Re-check each redirect hop against the SSRF blocklist: the initial-URL check in
            // icons::favicon can't see a public host redirecting to 169.254.169.254 etc. `stop()`
            // hands back the 3xx response, which the image-sniffing callers reject as not-an-image.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.url().host_str().map(is_blocked_host).unwrap_or(true) {
                    attempt.stop()
                } else if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::is_blocked_host;

    #[test]
    fn blocks_resolver_short_and_hex_ip_forms() {
        assert!(is_blocked_host("127.0.0.1.")); // trailing-dot absolute form
        assert!(is_blocked_host("127.1")); // inet_aton short form
        assert!(is_blocked_host("0x7f.0.0.1")); // hex-octet form
        assert!(!is_blocked_host("8.8.8.8")); // strict public IP still fine
        assert!(!is_blocked_host("spotify.com"));
    }
}
