// OmniDeck — mutex recovery with a breadcrumb.
//
// The launcher's mutexes guard simple caches (live process groups, hidden windows, MPRIS
// player state). A panic that poisons one is close to impossible-by-construction, but the
// old `.lock().ok()` / `if let Ok(..)` pattern meant that if it DID happen, every later
// critical section silently no-op'd — a half-working launcher with zero diagnostic (audit
// finding). Recover the guard instead (the data is a rebuildable cache) and leave a warning
// so the session log tells the story.
use std::sync::{Mutex, MutexGuard};

pub fn lock_or_recover<'a, T>(m: &'a Mutex<T>, what: &str) -> MutexGuard<'a, T> {
    m.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("{what}: mutex poisoned by an earlier panic — recovering");
        poisoned.into_inner()
    })
}
