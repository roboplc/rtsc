#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[path = "linux_gnu.rs"]
mod os;

#[cfg(all(target_os = "linux", not(target_env = "gnu")))]
#[path = "linux_other.rs"]
mod os;

#[cfg(not(target_os = "linux"))]
#[path = "unsupported.rs"]
mod os;

pub use os::{apply, apply_for_current};

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
