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

/// Thread scheduler, CPU affinity and heap parameters
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Params {
    /// Thread priority
    priority: Option<i32>,
    /// Thread scheduler policy
    scheduling: Scheduling,
    /// CPU affinity
    cpu_ids: Vec<usize>,
}

impl Params {
    /// Create a new instance of the parameters
    pub fn new() -> Self {
        Self::default()
    }
    /// Set the thread priority
    pub fn with_priority(mut self, priority: Option<i32>) -> Self {
        self.priority = priority;
        self
    }
    /// Set the thread scheduler policy
    pub fn with_scheduling(mut self, scheduling: Scheduling) -> Self {
        self.scheduling = scheduling;
        self
    }
    /// Set the CPU affinity
    pub fn with_cpu_ids(mut self, cpu_ids: &[usize]) -> Self {
        self.cpu_ids = cpu_ids.to_vec();
        self
    }
    /// Get the thread priority
    pub fn priority(&self) -> Option<i32> {
        self.priority
    }
    /// Get the thread scheduler policy
    pub fn scheduling(&self) -> Scheduling {
        self.scheduling
    }
    /// Get the CPU affinity
    pub fn cpu_ids(&self) -> &[usize] {
        &self.cpu_ids
    }
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

/// Apply the thread scheduler and CPU affinity parameters for a given thread. heap preallocation
/// is ignored
#[inline]
pub fn apply(tid: libc::c_int, params: &Params) -> Result<()> {
    os::apply(tid, params)
}

/// The method preallocates a heap memory region with the given size. The method is useful to
/// prevent memory fragmentation and speed up memory allocation. It is highly recommended to call
/// the method at the beginning of the program.
///
/// Does nothing in simulated mode.
///
/// # Panics
///
/// Will panic if the page size is too large (more than usize)
#[inline]
pub fn preallocate_heap(size: usize) -> Result<()> {
    os::prealloc_heap(size)
}
