use std::time::Duration;

use bma_ts::Monotonic;

use crate::Error;

/// A time-limited operation
pub struct Operation {
    t: Monotonic,
    timeout: Duration,
}

impl Operation {
    /// Create a new operation with the given timeout
    pub fn new(timeout: Duration) -> Self {
        Self {
            t: Monotonic::now(),
            timeout,
        }
    }
    /// Create a new operation with the given timeout and starting time
    pub fn new_for_monotonic(t: Monotonic, timeout: Duration) -> Self {
        Self { t, timeout }
    }
    /// Return the starting monotonic time of the operation
    pub fn started_at(&self) -> Monotonic {
        self.t
    }
    /// Check if the operation has timed out and return remaining duration
    pub fn remaining(&self) -> Result<Duration, Error> {
        let elapsed = self.t.elapsed();
        if elapsed > self.timeout {
            Err(Error::Timeout)
        } else {
            Ok(self.timeout - elapsed)
        }
    }
    /// Check if the operation has timed out and return remaining duration for a specific timeout
    pub fn remaining_for(&self, timeout: Duration) -> Result<Duration, Error> {
        let elapsed = self.t.elapsed();
        if elapsed > timeout {
            Err(Error::Timeout)
        } else {
            Ok(timeout - elapsed)
        }
    }
    /// Check if the operation has enough time for the expected duration
    pub fn enough(&self, expected: Duration) -> Result<(), Error> {
        if self.t.elapsed() + expected <= self.timeout {
            Ok(())
        } else {
            Err(Error::Timeout)
        }
    }
}
