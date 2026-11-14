// OmniDeck — parental-controls PIN (roadmap #2).
//
// Stores an argon2 hash of the PIN in Settings (`pin_hash`, empty = no lock) and
// verifies attempts against it. NEVER stores plaintext.
//
// Threat model (per NOTES-DEEPDIVE-ROADMAP.md §2): this is **deterrence, not access
// control**. The launcher runs as the user — anyone with shell access can read
// config.toml or launch Steam directly. The gate keeps kids on the couch out of
// locked categories; it does not sandbox anything.
//
// Both commands run the argon2 work (deliberately slow by design) on the blocking
// pool via `tauri::async_runtime::spawn_blocking`, so a verify never janks the
// UI/gamepad thread.
use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config;

/// Anti-hammering state shared by every PIN-checking command. Without it, argon2's cost is
/// the only brake and a scripted caller walks all 10,000 4-digit candidates in minutes —
/// "deterrence" (the module's stated bar) requires throttling automated guessing too.
/// In-memory only: a restart clears it, which matches the couch threat model.
static ATTEMPTS: Mutex<AttemptState> = Mutex::new(AttemptState { failures: 0, locked_until: None });

struct AttemptState {
    failures: u32,
    locked_until: Option<Instant>,
}

const LOCKOUT_AFTER: u32 = 5;

/// How long attempt N locks out for: nothing before LOCKOUT_AFTER consecutive failures,
/// then 30 s doubling per further failure, capped at 15 min. Pure for testability.
fn lockout_duration(failures: u32) -> Option<Duration> {
    if failures < LOCKOUT_AFTER {
        return None;
    }
    let exp = (failures - LOCKOUT_AFTER).min(5);
    Some((Duration::from_secs(30) * 2u32.pow(exp)).min(Duration::from_secs(15 * 60)))
}

/// Err while a lockout window is active (message includes the remaining seconds).
fn check_locked() -> Result<(), String> {
    let st = crate::sync::lock_or_recover(&ATTEMPTS, "pin.ATTEMPTS");
    if let Some(t) = st.locked_until {
        let now = Instant::now();
        if now < t {
            let secs = (t - now).as_secs() + 1;
            return Err(format!("too many wrong PINs — try again in {secs}s"));
        }
    }
    Ok(())
}

/// Record a verification outcome: success resets the counter, failure advances it and
/// (past the threshold) arms the next lockout window.
fn record_outcome(ok: bool) {
    let mut st = crate::sync::lock_or_recover(&ATTEMPTS, "pin.ATTEMPTS");
    if ok {
        st.failures = 0;
        st.locked_until = None;
        return;
    }
    st.failures += 1;
    if let Some(d) = lockout_duration(st.failures) {
        st.locked_until = Some(Instant::now() + d);
    }
}

/// The one wrong-PIN message — commands match on it to know a failure was a bad PIN
/// (throttle-relevant) rather than some other error.
const WRONG_PIN: &str = "current PIN is incorrect";

/// Hash a PIN with argon2id and a fresh random salt (PHC string format).
fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("could not hash PIN: {e}"))
}

/// True when `pin` matches the stored PHC hash. An empty/unparseable hash never matches.
fn pin_matches(stored_hash: &str, pin: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false; // empty ("no lock") or corrupted hash — treat as no match
    };
    Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok()
}

/// Pure PIN gate: Ok when no PIN is set, or when `pin` is provided and matches the
/// stored hash. Shared by every mutation that must be PIN-authorized.
fn require_pin(stored_hash: &str, pin: Option<&str>) -> Result<(), String> {
    if stored_hash.is_empty() {
        return Ok(()); // no lock configured
    }
    match pin {
        Some(p) if pin_matches(stored_hash, p) => Ok(()),
        _ => Err(WRONG_PIN.into()),
    }
}

/// Pure set-PIN transition: given the stored hash, decide the next one.
/// - A PIN is already set (non-empty hash): `current` must be provided and correct.
/// - No PIN set: `current` is ignored.
/// - Empty `new` clears the PIN (Settings docs: empty hash = no lock).
fn next_pin_hash(stored_hash: &str, current: Option<&str>, new: &str) -> Result<String, String> {
    require_pin(stored_hash, current)?;
    if new.is_empty() {
        return Ok(String::new()); // clear the lock
    }
    hash_pin(new)
}

/// Set, change, or clear the PIN. When a PIN already exists, `current` must match it
/// before anything changes. An empty `new` clears the lock. Persists via the shared
/// config save path.
#[tauri::command]
pub async fn set_pin(current: Option<String>, new: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_locked()?;
        let stored = config::load_or_create().settings.pin_hash;
        let gated = !stored.is_empty(); // no PIN configured = nothing to hammer
        let next = match next_pin_hash(&stored, current.as_deref(), &new) {
            Ok(n) => {
                if gated {
                    record_outcome(true);
                }
                n
            }
            Err(e) => {
                if gated && e == WRONG_PIN {
                    record_outcome(false);
                }
                return Err(e);
            }
        };
        config::save_pin_hash(next)
    })
    .await
    .map_err(|e| format!("pin task failed: {e}"))?
}

/// Replace the PIN-locked category ids. When a PIN is set, `pin` must match it — the
/// server-authoritative counterpart to `save_settings` preserving `locked_categories`,
/// so a plain settings save can't unlock everything without knowing the PIN.
#[tauri::command]
pub async fn set_locked_categories(pin: Option<String>, categories: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_locked()?;
        let stored = config::load_or_create().settings.pin_hash;
        let r = require_pin(&stored, pin.as_deref());
        if !stored.is_empty() {
            record_outcome(r.is_ok());
        }
        r?;
        config::save_locked_categories(categories)
    })
    .await
    .map_err(|e| format!("pin task failed: {e}"))?
}

/// True when `pin` matches the stored PIN hash. Always false when no PIN is set.
#[tauri::command]
pub async fn verify_pin(pin: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        if check_locked().is_err() {
            return false; // locked out — don't even run the verify (no oracle while locked)
        }
        let stored = config::load_or_create().settings.pin_hash;
        if stored.is_empty() {
            return false; // no PIN configured: not a guess, don't count it
        }
        let ok = pin_matches(&stored, &pin);
        record_outcome(ok);
        ok
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{hash_pin, lockout_duration, next_pin_hash, pin_matches, require_pin};
    use std::time::Duration;

    #[test]
    fn lockout_schedule_arms_after_threshold_and_caps() {
        // Below the threshold: no lockout (a fat-fingered couch PIN shouldn't punish).
        for f in 0..5 {
            assert_eq!(lockout_duration(f), None);
        }
        // At and past the threshold: 30 s doubling per failure, capped at 15 min.
        assert_eq!(lockout_duration(5), Some(Duration::from_secs(30)));
        assert_eq!(lockout_duration(6), Some(Duration::from_secs(60)));
        assert_eq!(lockout_duration(10), Some(Duration::from_secs(900))); // 30*32=960 → cap
        assert_eq!(lockout_duration(1000), Some(Duration::from_secs(900)));
    }

    #[test]
    fn require_pin_gates_mutations() {
        // No PIN configured: everything is allowed, with or without a pin argument.
        assert!(require_pin("", None).is_ok());
        assert!(require_pin("", Some("1234")).is_ok());
        // PIN configured: only the correct PIN passes — missing/wrong/empty are rejected,
        // so locked_categories cannot be changed by a plain settings payload.
        let stored = hash_pin("1234").expect("hash");
        assert!(require_pin(&stored, Some("1234")).is_ok());
        assert!(require_pin(&stored, None).is_err());
        assert!(require_pin(&stored, Some("0000")).is_err());
        assert!(require_pin(&stored, Some("")).is_err());
    }

    #[test]
    fn set_then_verify_roundtrip() {
        let hash = next_pin_hash("", None, "1234").expect("set");
        assert!(hash.starts_with("$argon2"), "PHC-format argon2 hash, never plaintext");
        assert!(!hash.contains("1234"), "hash must not embed the plaintext PIN");
        assert!(pin_matches(&hash, "1234"));
    }

    #[test]
    fn wrong_pin_rejects() {
        let hash = hash_pin("1234").expect("hash");
        assert!(!pin_matches(&hash, "4321"));
        assert!(!pin_matches(&hash, ""));
        // No lock / corrupted hash never verifies.
        assert!(!pin_matches("", "1234"));
        assert!(!pin_matches("not-a-phc-hash", "1234"));
    }

    #[test]
    fn changing_pin_requires_correct_current() {
        let stored = hash_pin("1234").expect("hash");
        // Wrong or missing current PIN → rejected, hash unchanged by the caller.
        assert!(next_pin_hash(&stored, Some("0000"), "5678").is_err());
        assert!(next_pin_hash(&stored, None, "5678").is_err());
        // Correct current PIN → new hash verifies the new PIN only.
        let changed = next_pin_hash(&stored, Some("1234"), "5678").expect("change");
        assert!(pin_matches(&changed, "5678"));
        assert!(!pin_matches(&changed, "1234"));
    }

    #[test]
    fn clearing_pin_requires_correct_current() {
        let stored = hash_pin("1234").expect("hash");
        assert!(next_pin_hash(&stored, Some("9999"), "").is_err());
        let cleared = next_pin_hash(&stored, Some("1234"), "").expect("clear");
        assert!(cleared.is_empty(), "empty hash = no lock");
    }

    #[test]
    fn salts_are_random() {
        // Same PIN twice → different hashes (fresh salt each time), both verify.
        let a = hash_pin("1234").unwrap();
        let b = hash_pin("1234").unwrap();
        assert_ne!(a, b);
        assert!(pin_matches(&a, "1234") && pin_matches(&b, "1234"));
    }
}
