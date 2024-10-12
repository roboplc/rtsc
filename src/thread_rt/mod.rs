use crate::Result;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[path = "linux_gnu.rs"]
mod os;

#[cfg(all(target_os = "linux", not(target_env = "gnu")))]
#[path = "linux_other.rs"]
mod os;

#[cfg(not(target_os = "linux"))]
#[path = "unsupported.rs"]
mod os;

/// Thread scheduler and CPU affinity parameters
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Params {
    priority: Option<i32>,
    scheduling: Scheduling,
    cpu_ids: Vec<usize>,
}

/// Scheduling policy (Linux)
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Scheduling {
    /// Round-robin
    RoundRobin,
    /// First in, first out
    FIFO,
    /// Idle
    Idle,
    /// Batch
    Batch,
    /// Deadline
    DeadLine,
    #[default]
    /// Other
    Other,
}

/// Apply the thread scheduler and CPU affinity parameters for the current thread
#[inline]
pub fn apply_for_current(params: &Params) -> Result<()> {
    os::apply_for_current(params)
}

/// Apply the thread scheduler and CPU affinity parameters
#[inline]
pub fn apply(tid: libc::c_int, params: &Params) -> Result<()> {
    os::apply(tid, params)
}
