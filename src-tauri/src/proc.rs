// OmniDeck — tiny child-process helpers shared by modules that shell out.
use std::io::Read;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

/// `Command::output()` with a deadline: spawn, drain stdout on a reader thread, poll
/// `try_wait`, and KILL the child when the deadline passes (returning None). Plain
/// `.output()` waits forever, and the callers here sit on paths that must not hang — the
/// deck-open freeze policy (pactl against a wedged PipeWire) and the first-play mpv
/// capability probe (stalled NFS binary). The reader thread drains concurrently so a chatty
/// child (pactl with dozens of sink-inputs) can't block on a full pipe and eat the deadline.
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    // stdout is always Some here: we set Stdio::piped() above.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let status: ExitStatus = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait(); // reap — no zombie
                let _ = reader.join(); // kill closed the pipe — reader sees EOF
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait(); // reap — no zombie
                let _ = reader.join();
                return None;
            }
        }
    };
    let stdout = reader.join().ok()?;
    Some(Output { status, stdout, stderr: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::output_with_timeout;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn fast_child_completes_and_slow_child_is_killed() {
        let mut ok = Command::new("sh");
        ok.args(["-c", "echo hi"]);
        let out = output_with_timeout(ok, Duration::from_secs(5)).expect("echo runs");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");

        let mut slow = Command::new("sleep");
        slow.arg("30");
        let start = std::time::Instant::now();
        assert!(output_with_timeout(slow, Duration::from_millis(200)).is_none());
        assert!(start.elapsed() < Duration::from_secs(5), "timeout must not wait for the child");
    }

    #[test]
    fn output_larger_than_the_pipe_buffer_does_not_deadlock() {
        // A child emitting more than the ~64 KiB pipe buffer must complete, not block on a
        // full pipe until the deadline kills it.
        let mut big = Command::new("sh");
        big.args(["-c", "head -c 200000 /dev/zero"]);
        let out = output_with_timeout(big, Duration::from_secs(5)).expect("big output completes");
        assert_eq!(out.stdout.len(), 200_000);
    }
}
