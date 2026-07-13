// OmniDeck — couch sleep timer: "stop playback in N minutes" (feature-backlog parking lot).
//
// Arming spawns one async task that polls the shared armed-state once a second (the same
// cheap-tick shape as the watchdog's poll loops — no wheel/heap timer to cancel). Re-arming
// REPLACES the running timer: every `set` bumps a process-wide generation counter, and a
// task whose generation no longer matches the armed state simply exits on its next tick, so
// cancel/re-arm never race the expiry action. On expiry the task pauses every Playing MPRIS
// player (pause, not kill — resuming after "just one more episode" must be one button), then
// emits `sleep-timer-fired`; during the final minute it emits `sleep-timer-tick` (remaining
// seconds) once a second so the frontend can warn/dim before the cut.
//
// DELIBERATELY not persisted across restarts: a sleep timer is a statement about *tonight's*
// session — rehydrating one from config after a morning boot would pause playback out of
// nowhere. If the launcher restarts, the timer is gone; that's the honest behavior.
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Seconds before expiry during which `sleep-timer-tick` events are emitted.
const TICK_WINDOW_SECS: u64 = 60;
/// Arming bounds: at least a minute, at most a day (couch presets are 15–90 min).
const MIN_MINUTES: u32 = 1;
const MAX_MINUTES: u32 = 24 * 60;

pub const TICK_EVENT: &str = "sleep-timer-tick";
pub const FIRED_EVENT: &str = "sleep-timer-fired";

/// What `get_sleep_timer` returns while a timer is armed (and `set_sleep_timer` on arm).
/// `total_secs` lets the UI draw a progress ring without remembering what it asked for.
#[derive(Clone, Copy, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SleepTimerStatus {
    pub remaining_secs: u32,
    pub total_secs: u32,
}

#[derive(Clone, Copy)]
struct Armed {
    /// Ownership token: the task spawned for this arming exits when the slot's generation
    /// moves on (re-arm) or the slot empties (cancel). Never reused within a process.
    generation: u64,
    deadline: Instant,
    total_secs: u32,
}

static ARMED: Mutex<Option<Armed>> = Mutex::new(None);
/// Monotonic and never reset — a cancel+re-arm inside one poll interval must not hand a
/// stale task a generation it recognizes.
static NEXT_GEN: AtomicU64 = AtomicU64::new(0);

/// Whole seconds until `deadline`, rounded UP (a freshly-armed 15-minute timer reads 900,
/// not 899; sub-second remainders read 1, not a premature 0 — 0 strictly means "expired").
fn remaining_secs(deadline: Instant, now: Instant) -> u64 {
    let d = deadline.saturating_duration_since(now);
    d.as_secs() + u64::from(d.subsec_nanos() > 0)
}

/// Validate + write the armed state. Pure state transition (no spawn) so re-arm semantics
/// are unit-testable against a local slot.
fn arm(slot: &Mutex<Option<Armed>>, minutes: u32, now: Instant, generation: u64) -> Result<Armed, String> {
    if !(MIN_MINUTES..=MAX_MINUTES).contains(&minutes) {
        return Err(format!("sleep timer must be {MIN_MINUTES}–{MAX_MINUTES} minutes, got {minutes}"));
    }
    let total_secs = minutes * 60;
    let armed = Armed { generation, deadline: now + Duration::from_secs(u64::from(total_secs)), total_secs };
    *crate::sync::lock_or_recover(slot, "sleep_timer.armed") = Some(armed);
    Ok(armed)
}

/// Clear the armed state; true if a timer was actually running. The task notices on its
/// next 1 s tick and exits — nothing to join.
fn disarm(slot: &Mutex<Option<Armed>>) -> bool {
    crate::sync::lock_or_recover(slot, "sleep_timer.armed").take().is_some()
}

fn status(slot: &Mutex<Option<Armed>>, now: Instant) -> Option<SleepTimerStatus> {
    let a = (*crate::sync::lock_or_recover(slot, "sleep_timer.armed"))?;
    Some(SleepTimerStatus {
        remaining_secs: remaining_secs(a.deadline, now).min(u64::from(u32::MAX)) as u32,
        total_secs: a.total_secs,
    })
}

/// Arm (or re-arm — replaces any running timer) and spawn the expiry task.
pub fn set(app: tauri::AppHandle, minutes: u32) -> Result<SleepTimerStatus, String> {
    let generation = NEXT_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let armed = arm(&ARMED, minutes, Instant::now(), generation)?;
    tracing::info!(minutes, generation, "sleep timer armed");
    tauri::async_runtime::spawn(run(app, generation));
    Ok(SleepTimerStatus { remaining_secs: armed.total_secs, total_secs: armed.total_secs })
}

/// Cancel the running timer; false when none was armed (idempotent, the UI needn't care).
pub fn cancel() -> bool {
    let was = disarm(&ARMED);
    if was {
        tracing::info!("sleep timer cancelled");
    }
    was
}

/// Current status, or None when no timer is armed.
pub fn get() -> Option<SleepTimerStatus> {
    status(&ARMED, Instant::now())
}

/// The armed timer's task: 1 s poll until expiry, tick events over the last minute, then
/// pause-and-notify. Exits silently the moment its generation is superseded or cancelled.
async fn run(app: tauri::AppHandle, generation: u64) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let remaining = {
            let slot = crate::sync::lock_or_recover(&ARMED, "sleep_timer.armed");
            match slot.as_ref() {
                Some(a) if a.generation == generation => remaining_secs(a.deadline, Instant::now()),
                _ => return, // cancelled or re-armed — a newer task (or nobody) owns the timer
            }
        };
        if remaining == 0 {
            break;
        }
        if remaining <= TICK_WINDOW_SECS {
            let _ = app.emit(TICK_EVENT, remaining as u32);
        }
    }
    // Expired. Clear the slot IF still ours — a re-arm landing in the final second wins,
    // and its task (not this one) will do the acting.
    {
        let mut slot = crate::sync::lock_or_recover(&ARMED, "sleep_timer.armed");
        match slot.as_ref() {
            Some(a) if a.generation == generation => *slot = None,
            _ => return,
        }
    }
    // Pause, don't kill: every Playing MPRIS player gets Pause() (native players, browser
    // PWAs, and mpv when its MPRIS plugin is present — see mpris::pause_all). The payload
    // is how many players were paused, so the frontend can word its toast honestly.
    let paused = crate::mpris::pause_all().await;
    tracing::info!(paused, "sleep timer fired");
    let _ = app.emit(FIRED_EVENT, paused as u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_rounds_up_and_saturates() {
        let now = Instant::now();
        assert_eq!(remaining_secs(now + Duration::from_secs(900), now), 900);
        // sub-second remainder reads 1 (still armed), never a premature 0
        assert_eq!(remaining_secs(now + Duration::from_millis(400), now), 1);
        assert_eq!(remaining_secs(now + Duration::from_millis(60_500), now), 61);
        // past-deadline saturates at 0 — 0 strictly means "expired"
        assert_eq!(remaining_secs(now, now), 0);
        assert_eq!(remaining_secs(now, now + Duration::from_secs(5)), 0);
    }

    #[test]
    fn arm_validates_minutes() {
        let slot = Mutex::new(None);
        let now = Instant::now();
        assert!(arm(&slot, 0, now, 1).is_err());
        assert!(arm(&slot, MAX_MINUTES + 1, now, 1).is_err());
        assert!(slot.lock().unwrap().is_none()); // a rejected arm must not touch the slot
        assert!(arm(&slot, MIN_MINUTES, now, 1).is_ok());
        assert!(arm(&slot, MAX_MINUTES, now, 2).is_ok());
    }

    #[test]
    fn rearm_replaces_deadline_and_generation() {
        let slot = Mutex::new(None);
        let now = Instant::now();
        arm(&slot, 90, now, 1).unwrap();
        // Re-arm SHORTER: the new deadline must rule even though it's earlier.
        let a = arm(&slot, 15, now, 2).unwrap();
        assert_eq!(a.generation, 2);
        assert_eq!(a.total_secs, 15 * 60);
        let st = status(&slot, now).unwrap();
        assert_eq!(st.remaining_secs, 15 * 60);
        assert_eq!(st.total_secs, 15 * 60);
        // The generation-1 task's ownership check now fails — exactly how it learns to exit.
        assert_ne!(slot.lock().unwrap().unwrap().generation, 1);
    }

    #[test]
    fn cancel_clears_and_is_idempotent() {
        let slot = Mutex::new(None);
        assert!(!disarm(&slot)); // nothing armed
        arm(&slot, 30, Instant::now(), 7).unwrap();
        assert!(disarm(&slot));
        assert!(status(&slot, Instant::now()).is_none());
        assert!(!disarm(&slot)); // second cancel is a no-op
    }

    #[test]
    fn status_counts_down_against_now() {
        let slot = Mutex::new(None);
        let now = Instant::now();
        arm(&slot, 15, now, 1).unwrap();
        assert_eq!(status(&slot, now).unwrap().remaining_secs, 900);
        let later = now + Duration::from_secs(880);
        let st = status(&slot, later).unwrap();
        assert_eq!(st.remaining_secs, 20); // inside the tick window
        assert_eq!(st.total_secs, 900); // total is the armed duration, not remaining
        assert_eq!(status(&slot, now + Duration::from_secs(2000)).unwrap().remaining_secs, 0);
    }
}
