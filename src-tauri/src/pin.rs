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

use crate::config;

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

/// Pure set-PIN transition: given the stored hash, decide the next one.
/// - A PIN is already set (non-empty hash): `current` must be provided and correct.
/// - No PIN set: `current` is ignored.
/// - Empty `new` clears the PIN (Settings docs: empty hash = no lock).
fn next_pin_hash(stored_hash: &str, current: Option<&str>, new: &str) -> Result<String, String> {
    if !stored_hash.is_empty() {
        match current {
            Some(c) if pin_matches(stored_hash, c) => {}
            _ => return Err("current PIN is incorrect".into()),
        }
    }
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
        let stored = config::load_or_create().settings.pin_hash;
        let next = next_pin_hash(&stored, current.as_deref(), &new)?;
        config::save_pin_hash(next)
    })
    .await
    .map_err(|e| format!("pin task failed: {e}"))?
}

/// True when `pin` matches the stored PIN hash. Always false when no PIN is set.
#[tauri::command]
pub async fn verify_pin(pin: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        let stored = config::load_or_create().settings.pin_hash;
        pin_matches(&stored, &pin)
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{hash_pin, next_pin_hash, pin_matches};

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
