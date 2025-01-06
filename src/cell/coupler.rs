use std::{sync::Arc, time::Duration};

use crate::{
    condvar_api::RawCondvar,
    locking::{Condvar, RawMutex},
    Error, Result,
};
use lock_api::RawMutex as RawMutexTrait;

struct CellValue<P, S> {
    primary: Option<P>,
    second: Option<S>,
    closed: bool,
}

impl<P, S> Default for CellValue<P, S> {
    fn default() -> Self {
        Self {
            primary: None,
            second: None,
            closed: false,
        }
    }
}

struct CouplerInner<P, S, M, CV> {
    value: lock_api::Mutex<M, CellValue<P, S>>,
    data_available: CV,
}

/// Data coupler, which combines [`crate::cell::DataCell`] functionality with a secondary data
/// value. The primary value is combined with the secondary, the secondary may not be present.
pub struct Coupler<P, S, M = RawMutex, CV = Condvar> {
    inner: Arc<CouplerInner<P, S, M, CV>>,
}

impl<P, S, M, CV> Default for Coupler<P, S, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn default() -> Self {
        Self {
            inner: Arc::new(CouplerInner {
                value: <_>::default(),
                data_available: CV::new(),
            }),
        }
    }
}

impl<P, S, M, CV> Clone for Coupler<P, S, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P, S, M, CV> Coupler<P, S, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar + RawCondvar<RawMutex = M>,
{
    /// Creates a new coupler
    pub fn new() -> Self {
        Self::default()
    }
    /// Closes the cell, preventing any further data from being retrieved
    pub fn close(&self) {
        let mut value = self.inner.value.lock();
        value.closed = true;
        self.inner.data_available.notify_all();
    }
    /// Returns true if the cell is closed
    pub fn is_closed(&self) -> bool {
        let value = self.inner.value.lock();
        value.closed
    }
    /// Sets the primary value
    pub fn set(&self, data: P) {
        let mut value = self.inner.value.lock();
        value.primary = Some(data);
        self.inner.data_available.notify_one();
    }
    /// Replaces the primary value and returns the old one if any
    pub fn replace(&self, data: P) -> Option<P> {
        let mut value = self.inner.value.lock();
        let prev = value.primary.replace(data);
        self.inner.data_available.notify_one();
        prev
    }
    /// Sets the second value
    pub fn set_second(&self, data: S) {
        let mut value = self.inner.value.lock();
        value.second = Some(data);
    }
    /// Replaces the second value and returns the old one if any
    pub fn replace_second(&self, data: S) -> Option<S> {
        let mut value = self.inner.value.lock();
        value.second.replace(data)
    }
    /// Retrieves the primary and secondary values from the cell
    pub fn get(&self) -> Result<(P, Option<S>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(primary) = value.primary.take() {
                return Ok((primary, value.second.take()));
            }
            self.inner
                .data_available
                .wait::<CellValue<P, S>, M>(&mut value);
        }
    }
    /// Retrieves the primary and secondary values from the cell with the given timeout
    pub fn get_timeout(&self, timeout: Duration) -> Result<(P, Option<S>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(primary) = value.primary.take() {
                return Ok((primary, value.second.take()));
            }
            if self
                .inner
                .data_available
                .wait_for::<CellValue<P, S>, M>(&mut value, timeout)
                .timed_out()
            {
                return Err(Error::Timeout);
            }
        }
    }
    /// Tries to retrieve the data from the cell (non-blocking)
    pub fn try_get(&self) -> Result<(P, Option<S>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        let primary = value.primary.take().ok_or(Error::ChannelEmpty)?;
        Ok((primary, value.second.take()))
    }
}

impl<P, S> Iterator for Coupler<P, S> {
    type Item = (P, Option<S>);
    fn next(&mut self) -> Option<Self::Item> {
        self.get().ok()
    }
}

#[cfg(test)]
mod test {

    use crate::Error;

    use super::Coupler;
    use std::{thread, time::Duration};

    #[test]
    fn test_coupler() {
        let cell: Coupler<_, _> = Coupler::new();
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            cell2.set_second(33);
            cell2.set(42);
        });
        assert_eq!(cell.get().unwrap(), (42, Some(33)));
        handle.join().unwrap();
    }

    #[test]
    fn test_coupler_close() {
        let cell = Coupler::<usize, usize>::new();
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            cell2.set(42);
        });
        cell.close();
        assert!(matches!(cell.get().unwrap_err(), Error::ChannelClosed));
        handle.join().unwrap();
    }

    #[test]
    fn test_coupler_try_get() {
        let cell: Coupler<_, _> = Coupler::new();
        assert!(matches!(cell.try_get().unwrap_err(), Error::ChannelEmpty));
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            cell2.set_second(33);
            cell2.set(42);
        });
        handle.join().unwrap();
        assert_eq!(cell.try_get().unwrap(), (42, Some(33)));
    }

    #[test]
    fn test_coupler_other_mutex() {
        let cell: Coupler<_, _, parking_lot_rt::RawMutex, parking_lot_rt::Condvar> = Coupler::new();
        let _cell2 = cell.clone();
        cell.set_second(33);
        cell.set(42);
        assert_eq!(cell.get().unwrap(), (42, Some(33)));
    }
}
