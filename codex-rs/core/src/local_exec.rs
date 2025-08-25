use std::sync::Mutex;

pub(crate) struct LocalExecRuntime {
    pgid: Mutex<Option<i32>>,
}

impl LocalExecRuntime {
    pub(crate) fn new() -> Self {
        Self {
            pgid: Mutex::new(None),
        }
    }
}

/// Configure child process before exec: on Unix, create a new process group so
/// we can signal the entire tree later.
pub(crate) fn configure_child(cmd: &mut tokio::process::Command) {
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }
}

/// Record the spawned child so future interrupts can target it.
pub(crate) fn record_child(runtime: &LocalExecRuntime, pid_opt: Option<u32>) {
    if let Some(pid_u32) = pid_opt {
        let pid = pid_u32 as i32;
        // If getpgid fails, fall back to pid.
        let pgid = unsafe { libc::getpgid(pid) };
        let value = if pgid > 0 { pgid } else { pid };
        if let Ok(mut guard) = runtime.pgid.lock() {
            *guard = Some(value);
        }
    }
}

/// Clear any recorded child state after it exits or upon spawn failure.
pub(crate) fn clear(runtime: &LocalExecRuntime) {
    if let Ok(mut guard) = runtime.pgid.lock() {
        *guard = None;
    }
}

/// Attempt to interrupt a recorded child process tree.
pub(crate) fn interrupt(runtime: &LocalExecRuntime) {
    if let Ok(mut guard) = runtime.pgid.lock()
        && let Some(pgid) = guard.take()
    {
        unsafe {
            let _ = libc::kill(-pgid, libc::SIGINT);
        }
    }
}
