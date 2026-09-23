// OmniDeck — termination signals: thaw frozen app groups before the process dies.
//
// The Tauri run loop's `RunEvent::Exit` hook (lib.rs) thaws every switcher-frozen group on
// a GRACEFUL exit (Quit tile, window close). Session teardown is not graceful: gamescope
// (via gamescopereaper) SIGTERMs its client, and the default disposition kills the process
// on the spot — the Exit hook never runs, and every SIGSTOPped hidden app is stranded.
// A stopped X client can't notice its display going away, so gamescope then sits waiting
// on it (couch box 2026-09-15: a hung teardown behind a frozen, hidden PWA that a host
// script had to thaw by hand).
//
// Handler discipline: the signal handler itself only writes one byte to a pipe (async-
// signal-safe); a dedicated thread blocks on the read end and does the real work — the
// thaw takes locks and reads /proc, none of which may run inside a handler. After the
// callback the signal is re-raised with the default disposition restored, so the process
// still dies "by SIGTERM" exactly as the sender expects (exit status included).
use std::sync::atomic::{AtomicI32, Ordering};

/// Write end of the self-pipe; -1 until `install` ran.
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(sig: libc::c_int) {
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let b = sig as u8;
        // SAFETY: write(2) on a valid fd with a 1-byte buffer; async-signal-safe by POSIX.
        unsafe { libc::write(fd, std::ptr::from_ref(&b).cast::<libc::c_void>(), 1) };
    }
}

/// Install once, early (before any launched app can be frozen). `on_terminate` runs on a
/// dedicated thread with the signal number, then the signal is re-raised with its default
/// action. Returns false — logging why — if the pipe or handler setup failed; the graceful
/// exit path is unaffected either way.
pub fn install(on_terminate: impl FnOnce(i32) + Send + 'static) -> bool {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: pipe2 fills the two-element array; O_CLOEXEC so launched apps never inherit it.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        tracing::warn!(error = %std::io::Error::last_os_error(), "signals: pipe2 failed — no teardown thaw");
        return false;
    }
    let (rd, wr) = (fds[0], fds[1]);
    WAKE_FD.store(wr, Ordering::Relaxed);
    for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: a zeroed sigaction is a valid "no flags, empty mask" starting point; the
        // handler is an `extern "C" fn(c_int)` as sa_sigaction expects without SA_SIGINFO.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            if libc::sigaction(sig, &sa, std::ptr::null_mut()) != 0 {
                tracing::warn!(sig, error = %std::io::Error::last_os_error(), "signals: sigaction failed");
                return false;
            }
        }
    }
    let spawned = std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut b = 0u8;
            loop {
                // SAFETY: read(2) into a 1-byte buffer on the pipe's read end we own.
                let n =
                    unsafe { libc::read(rd, std::ptr::from_mut(&mut b).cast::<libc::c_void>(), 1) };
                if n == 1 {
                    break;
                }
                if n < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                {
                    continue;
                }
                return; // EOF/error: pipe gone — nothing to wait for
            }
            let sig = i32::from(b);
            on_terminate(sig);
            // SAFETY: restoring the default disposition and re-raising is the standard "handle,
            // then die by the signal" idiom; both calls are valid for any catchable signal.
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
                libc::raise(sig);
            }
        });
    match spawned {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(error = %e, "signals: could not spawn the wait thread");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    /// The self-pipe round trip: a real signal delivered to this process reaches the callback
    /// with its number. SIGHUP is used because nothing else in a test run sends it; the
    /// callback deliberately does NOT re-raise (that only happens after it returns, and here
    /// it never returns — the test keeps the thread parked on the channel send).
    #[test]
    fn a_delivered_signal_reaches_the_callback() {
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(super::install(move |sig| {
            let _ = tx.send(sig);
            // Park forever: returning would restore SIG_DFL and re-raise, killing cargo test.
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }));
        // SAFETY: raise(2) delivers to this thread; our handler only writes to the pipe.
        unsafe { libc::raise(libc::SIGHUP) };
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(libc::SIGHUP)
        );
        // The wait thread is parked for good, so hand SIGINT/SIGTERM back to the default
        // action: Ctrl-C on the rest of this test run must still work.
        // SAFETY: restoring default dispositions is always valid.
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::signal(libc::SIGTERM, libc::SIG_DFL);
        }
    }
}
