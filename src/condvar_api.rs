use std::time::Duration;

use lock_api::{MutexGuard, RawMutex};

/// The `Condvar` type trait
pub trait RawCondvar {
    /// The mutex type
    type RawMutex: RawMutex;

    /// Create a new `Condvar`
    fn new() -> Self;
    /// Wait on the `Condvar`
    fn wait<T, M>(&self, mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>);
    /// Wait on the `Condvar` with a timeout
    fn wait_for<T, M>(
        &self,
        mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>,
        timeout: Duration,
    ) -> WaitTimeoutResult;
    /// Notify one waiter
    fn notify_one(&self);
    /// Notify all waiters
    fn notify_all(&self);
}

/// Result, returned by [`RawCondvar::wait_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTimeoutResult {
    timed_out: bool,
}

impl WaitTimeoutResult {
    /// Create a new [`WaitTimeoutResult`]
    pub fn new(timed_out: bool) -> Self {
        Self { timed_out }
    }
    /// Returns `true` if the wait timed out.
    pub fn timed_out(&self) -> bool {
        self.timed_out
    }
}

/// Implementation for the real-time `parking_lot` crate fork
impl RawCondvar for parking_lot_rt::Condvar {
    type RawMutex = parking_lot_rt::RawMutex;

    fn new() -> Self {
        parking_lot_rt::Condvar::new()
    }

    fn wait<T, M>(&self, mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>) {
        self.wait(mutex_guard);
    }

    fn wait_for<T, M>(
        &self,
        mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>,
        timeout: Duration,
    ) -> WaitTimeoutResult {
        WaitTimeoutResult::new(self.wait_for(mutex_guard, timeout).timed_out())
    }

    fn notify_one(&self) {
        self.notify_one();
    }

    fn notify_all(&self) {
        self.notify_all();
    }
}

#[cfg(feature = "parking_lot")]
/// Implementation for the generic `parking_lot` crate
impl RawCondvar for parking_lot::Condvar {
    type RawMutex = parking_lot::RawMutex;

    fn new() -> Self {
        parking_lot::Condvar::new()
    }

    fn wait<T, M>(&self, mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>) {
        self.wait(mutex_guard);
    }

    fn wait_for<T, M>(
        &self,
        mutex_guard: &mut MutexGuard<'_, Self::RawMutex, T>,
        timeout: Duration,
    ) -> WaitTimeoutResult {
        WaitTimeoutResult::new(self.wait_for(mutex_guard, timeout).timed_out())
    }

    fn notify_one(&self) {
        self.notify_one();
    }

    fn notify_all(&self) {
        self.notify_all();
    }
}
